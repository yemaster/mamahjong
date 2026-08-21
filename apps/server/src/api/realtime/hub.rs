use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use mahjong_core::UserId;
use tokio::sync::{Notify, broadcast};

use super::message::ChatMessageType;

/// Notices are wake-up signals; connections pull their own redacted events.
/// A shallow buffer is enough because falling behind only costs one extra pull.
const NOTICE_CAPACITY: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StreamNotice {
    Changed {
        version: u64,
        latest_sequence: u64,
    },
    Chat {
        seat: u8,
        message_type: ChatMessageType,
        content: String,
    },
}

type Streams = HashMap<String, broadcast::Sender<StreamNotice>>;
/// Stream name, then connection identifier, then the user behind it.
type Presence = HashMap<String, HashMap<String, UserId>>;
/// Connection identifier → (user_id, close signal).
type UserConnections = HashMap<String, (UserId, Arc<Notify>)>;
type OfflineSince = HashMap<(String, UserId), Instant>;
type StreamChangedAt = HashMap<String, Instant>;
type RetiredStreams = HashSet<String>;

/// Fan-out of per-stream change signals to the connections watching them.
#[derive(Clone)]
pub(crate) struct RealtimeHub {
    streams: Arc<Mutex<Streams>>,
    presence: Arc<Mutex<Presence>>,
    user_connections: Arc<Mutex<UserConnections>>,
    offline_since: Arc<Mutex<OfflineSince>>,
    stream_changed_at: Arc<Mutex<StreamChangedAt>>,
    retired_streams: Arc<Mutex<RetiredStreams>>,
}

impl RealtimeHub {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            streams: Arc::new(Mutex::new(HashMap::new())),
            presence: Arc::new(Mutex::new(HashMap::new())),
            user_connections: Arc::new(Mutex::new(HashMap::new())),
            offline_since: Arc::new(Mutex::new(HashMap::new())),
            stream_changed_at: Arc::new(Mutex::new(HashMap::new())),
            retired_streams: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn subscribe(&self, stream: &str) -> broadcast::Receiver<StreamNotice> {
        let mut streams = self.lock();
        if let Some(sender) = streams.get(stream) {
            return sender.subscribe();
        }
        let (sender, receiver) = broadcast::channel(NOTICE_CAPACITY);
        streams.insert(stream.to_owned(), sender);
        receiver
    }

    /// Drops the stream once its last connection is gone.
    pub(crate) fn publish(&self, stream: &str, notice: StreamNotice) {
        if matches!(notice, StreamNotice::Changed { version, .. } if version != 0) {
            self.stream_changed_at_lock()
                .insert(stream.to_owned(), Instant::now());
        }
        let mut streams = self.lock();
        let Some(sender) = streams.get(stream) else {
            return;
        };
        if sender.send(notice).is_err() {
            streams.remove(stream);
        }
    }

    /// Wakes every connection on a stream without claiming new events.
    ///
    /// Connections recompute presence from the registry, so the notice carries
    /// no presence payload of its own.
    pub(crate) fn wake(&self, stream: &str) {
        self.publish(
            stream,
            StreamNotice::Changed {
                version: 0,
                latest_sequence: 0,
            },
        );
    }

    /// Registers a connection as online on a stream until the guard is dropped.
    pub(crate) fn join(
        &self,
        stream: &str,
        connection_id: &str,
        user_id: &UserId,
    ) -> PresenceGuard {
        self.offline_since_lock()
            .remove(&(stream.to_owned(), user_id.clone()));
        self.presence_lock()
            .entry(stream.to_owned())
            .or_default()
            .insert(connection_id.to_owned(), user_id.clone());
        self.wake(stream);
        PresenceGuard {
            hub: self.clone(),
            stream: stream.to_owned(),
            connection_id: connection_id.to_owned(),
        }
    }

    #[must_use]
    pub(crate) fn online_users(&self, stream: &str) -> HashSet<UserId> {
        self.presence_lock()
            .get(stream)
            .map(|connections| connections.values().cloned().collect())
            .unwrap_or_default()
    }

    /// 曾经订阅过对局、当前却没有任何连接的玩家。
    #[must_use]
    pub(crate) fn offline_users(&self) -> Vec<(String, UserId)> {
        self.offline_since_lock().keys().cloned().collect()
    }

    /// 重新核对玩家仍然离线，并判断当前牌局状态是否已经稳定了指定时长。
    #[must_use]
    pub(crate) fn offline_action_ready(
        &self,
        stream: &str,
        user_id: &UserId,
        delay: Duration,
    ) -> Option<bool> {
        let offline_at = self
            .offline_since_lock()
            .get(&(stream.to_owned(), user_id.clone()))
            .copied()?;
        let changed_at = self
            .stream_changed_at_lock()
            .get(stream)
            .copied()
            .unwrap_or(offline_at);
        Some(offline_at.max(changed_at).elapsed() >= delay)
    }

    /// 对局结束后停止追踪它的离线托管；仍在线的连接可以继续收完最后一帧。
    pub(crate) fn retire_stream(&self, stream: &str) {
        self.offline_since_lock()
            .retain(|(held_stream, _), _| held_stream != stream);
        self.stream_changed_at_lock().remove(stream);
        if self.presence_lock().contains_key(stream) {
            self.retired_streams_lock().insert(stream.to_owned());
        }
    }

    /// Registers a connection so it can be force-closed when the user's
    /// sessions are revoked (e.g. login from another client).
    pub(crate) fn register_connection(&self, user_id: &UserId, connection_id: &str) -> Arc<Notify> {
        let notify = Arc::new(Notify::new());
        self.user_connections_lock()
            .insert(connection_id.to_owned(), (user_id.clone(), notify.clone()));
        notify
    }

    pub(crate) fn unregister_connection(&self, connection_id: &str) {
        self.user_connections_lock().remove(connection_id);
    }

    /// Wakes every live connection owned by `user_id` so it closes itself.
    ///
    /// Used when the user logs in from a new client — all other clients are
    /// kicked immediately instead of waiting for their next HTTP request.
    pub(crate) fn revoke_user_connections(&self, user_id: &UserId) {
        let to_notify: Vec<Arc<Notify>> = {
            let mut map = self.user_connections_lock();
            let mut removed = Vec::new();
            map.retain(|_conn_id, (uid, notify)| {
                if uid == user_id {
                    removed.push(notify.clone());
                    false
                } else {
                    true
                }
            });
            removed
        };
        for notify in to_notify {
            notify.notify_one();
        }
    }

    fn user_connections_lock(&self) -> MutexGuard<'_, UserConnections> {
        self.user_connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn leave(&self, stream: &str, connection_id: &str) {
        let mut presence = self.presence_lock();
        let Some(connections) = presence.get_mut(stream) else {
            return;
        };
        let Some(user_id) = connections.remove(connection_id) else {
            return;
        };
        let user_still_online = connections.values().any(|held| held == &user_id);
        let stream_empty = connections.is_empty();
        if stream_empty {
            presence.remove(stream);
        }
        drop(presence);
        let retired = self.retired_streams_lock().contains(stream);
        if !user_still_online && !retired {
            self.offline_since_lock()
                .entry((stream.to_owned(), user_id))
                .or_insert_with(Instant::now);
        }
        if stream_empty && retired {
            self.retired_streams_lock().remove(stream);
        }
    }

    /// A poisoned lock only means an unrelated thread panicked; the map stays valid.
    fn lock(&self) -> MutexGuard<'_, Streams> {
        self.streams.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn presence_lock(&self) -> MutexGuard<'_, Presence> {
        self.presence.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn offline_since_lock(&self) -> MutexGuard<'_, OfflineSince> {
        self.offline_since
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn stream_changed_at_lock(&self) -> MutexGuard<'_, StreamChangedAt> {
        self.stream_changed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn retired_streams_lock(&self) -> MutexGuard<'_, RetiredStreams> {
        self.retired_streams
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps a connection listed as online and announces its departure.
pub(crate) struct PresenceGuard {
    hub: RealtimeHub,
    stream: String,
    connection_id: String,
}

impl Drop for PresenceGuard {
    fn drop(&mut self) {
        self.hub.leave(&self.stream, &self.connection_id);
        self.hub.wake(&self.stream);
    }
}

impl Default for RealtimeHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mahjong_core::UserId;

    use super::{RealtimeHub, StreamNotice};

    const NOTICE: StreamNotice = StreamNotice::Changed {
        version: 2,
        latest_sequence: 7,
    };

    #[tokio::test]
    async fn subscribers_of_a_stream_share_its_notices() {
        let hub = RealtimeHub::new();
        let mut first = hub.subscribe("match_a");
        let mut second = hub.subscribe("match_a");
        let mut other = hub.subscribe("match_b");

        hub.publish("match_a", NOTICE.clone());

        assert_eq!(first.recv().await.expect("notice"), NOTICE);
        assert_eq!(second.recv().await.expect("notice"), NOTICE);
        assert!(other.try_recv().is_err());
    }

    #[tokio::test]
    async fn publishing_without_subscribers_is_a_no_op() {
        let hub = RealtimeHub::new();
        hub.publish("match_a", NOTICE.clone());

        let mut receiver = hub.subscribe("match_a");
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn presence_follows_the_lifetime_of_its_guard() {
        let hub = RealtimeHub::new();
        let mut watcher = hub.subscribe("match_a");
        let user = UserId::new();

        let guard = hub.join("match_a", "conn_1", &user);
        assert_eq!(hub.online_users("match_a"), [user.clone()].into());
        assert!(watcher.try_recv().is_ok(), "joining wakes the stream");

        drop(guard);
        assert!(hub.online_users("match_a").is_empty());
        assert!(watcher.try_recv().is_ok(), "leaving wakes the stream");
    }

    #[tokio::test]
    async fn one_user_stays_online_while_any_connection_remains() {
        let hub = RealtimeHub::new();
        let user = UserId::new();
        let first = hub.join("match_a", "conn_1", &user);
        let second = hub.join("match_a", "conn_2", &user);

        drop(first);
        assert_eq!(hub.online_users("match_a"), [user.clone()].into());
        assert!(hub.offline_users().is_empty());
        drop(second);
        assert!(hub.online_users("match_a").is_empty());
        assert_eq!(hub.offline_users(), vec![("match_a".to_owned(), user)]);
    }

    #[tokio::test]
    async fn reconnecting_cancels_offline_automation_immediately() {
        let hub = RealtimeHub::new();
        let user = UserId::new();
        let connection = hub.join("match_a", "conn_1", &user);
        drop(connection);

        assert_eq!(
            hub.offline_action_ready("match_a", &user, Duration::from_secs(1)),
            Some(false)
        );
        assert_eq!(
            hub.offline_action_ready("match_a", &user, Duration::ZERO),
            Some(true)
        );

        let resumed = hub.join("match_a", "conn_2", &user);
        assert_eq!(
            hub.offline_action_ready("match_a", &user, Duration::ZERO),
            None
        );
        drop(resumed);
    }

    #[tokio::test]
    async fn finished_streams_do_not_leave_offline_automation_entries() {
        let hub = RealtimeHub::new();
        let user = UserId::new();
        let connection = hub.join("match_a", "conn_1", &user);

        hub.retire_stream("match_a");
        drop(connection);

        assert!(hub.offline_users().is_empty());
    }

    #[tokio::test]
    async fn a_lagging_subscriber_recovers_at_the_newest_notice() {
        let hub = RealtimeHub::new();
        let mut receiver = hub.subscribe("match_a");
        for sequence in 1..=64 {
            hub.publish(
                "match_a",
                StreamNotice::Changed {
                    version: sequence,
                    latest_sequence: sequence,
                },
            );
        }

        assert!(matches!(
            receiver.recv().await,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
        ));
        let mut latest = 0;
        while let Ok(notice) = receiver.try_recv() {
            if let StreamNotice::Changed {
                latest_sequence, ..
            } = notice
            {
                latest = latest_sequence;
            }
        }
        assert_eq!(latest, 64, "the newest notice survives lagging");
    }
}
