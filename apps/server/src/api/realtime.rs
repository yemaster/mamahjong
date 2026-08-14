mod hub;
mod message;
mod ticket;
mod viewdelta;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use mahjong_core::{MatchId, UserId};
use mamahjong_application::{MatchEventPage, MatchProjection, SubmitGameCommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::{Notify, broadcast, mpsc};

use self::hub::PresenceGuard;
use self::message::{
    ChatMessageType, ClientChat, ClientCommand, ClientMessage, Hello, MAX_MESSAGE_BYTES,
    MAX_SUBSCRIPTIONS, MessageError, PROTOCOL, Resync, SeatPresence, ServerMessage, StreamState,
};
use super::auth::AuthenticatedUser;
use super::dto::MatchViewResponse;
use super::error::ApiError;
use crate::AppState;
use crate::clock::SeatCountdown;
use crate::token::random_token;

pub(crate) use hub::{RealtimeHub, StreamNotice};
pub(crate) use ticket::WsTickets;

/// Only match streams exist in v1.
const MATCH_STREAM_PREFIX: &str = "match_";

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const SEND_TIMEOUT: Duration = Duration::from_secs(5);

const MAX_CHAT_TEXT_CHARS: usize = 200;
const MAX_CHAT_EMOJI_BYTES: usize = 512;

/// Client messages allowed per second before the connection is closed.
const MAX_MESSAGES_PER_SECOND: u32 = 20;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/ws-tickets", post(issue_ticket))
        .route("/ws", get(connect))
}

#[must_use]
pub(crate) fn match_stream(match_id: &MatchId) -> String {
    format!("{MATCH_STREAM_PREFIX}{match_id}")
}

fn parse_match_stream(stream: &str) -> Option<MatchId> {
    MatchId::parse(stream.strip_prefix(MATCH_STREAM_PREFIX)?.to_owned()).ok()
}

#[derive(Serialize)]
struct TicketResponse {
    schema: &'static str,
    ticket: String,
    expires_in: u64,
}

async fn issue_ticket(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<axum::Json<TicketResponse>, ApiError> {
    let issued = state
        .ws_tickets()
        .issue(user.user().id().clone())
        .ok_or_else(ApiError::internal)?;
    Ok(axum::Json(TicketResponse {
        schema: "ws_ticket.v1",
        ticket: issued.ticket,
        expires_in: issued.expires_in,
    }))
}

#[derive(Deserialize)]
struct ConnectQuery {
    ticket: String,
}

async fn connect(
    State(state): State<AppState>,
    Query(query): Query<ConnectQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let user_id = state
        .ws_tickets()
        .consume(&query.ticket)
        .ok_or_else(ApiError::invalid_ticket)?;
    let connection_id = random_token().ok_or_else(ApiError::internal)?;
    Ok(upgrade
        .max_message_size(MAX_MESSAGE_BYTES * 4)
        .on_upgrade(move |socket| async move {
            if let Err(error) = Connection::run(socket, state, user_id, connection_id).await {
                tracing::debug!(%error, "实时连接结束");
            }
        }))
}

/// One subscribed stream and how far this connection has delivered it.
struct Subscription {
    stream: String,
    match_id: MatchId,
    cursor: u64,
    /// Last presence sent, so an unchanged one is not resent.
    presence: Vec<SeatPresence>,
    /// 这条订阅要视图快照与补丁，而不是事件帧。
    wants_view: bool,
    /// 上一份**确实发出去过**的视图，下一个补丁就从它算起。
    ///
    /// 视图是按观察者裁剪过的，四家各不相同，所以记在订阅上而不是对局上。
    view: Option<Value>,
}

struct Connection {
    state: AppState,
    user_id: UserId,
    connection_id: String,
    socket: WebSocket,
    subscriptions: Vec<Subscription>,
    /// Keeps this connection listed as online for as long as it lives.
    presence: Vec<PresenceGuard>,
    /// Fires when the user's sessions are revoked (login from another client).
    close_signal: Option<Arc<Notify>>,
}

impl Connection {
    async fn run(
        socket: WebSocket,
        state: AppState,
        user_id: UserId,
        connection_id: String,
    ) -> Result<(), ConnectionError> {
        let close_signal = state
            .realtime()
            .register_connection(&user_id, &connection_id);
        let mut connection = Self {
            state,
            user_id,
            connection_id: connection_id.clone(),
            socket,
            subscriptions: Vec::new(),
            presence: Vec::new(),
            close_signal: Some(close_signal),
        };
        let outcome = connection.serve().await;
        connection
            .state
            .realtime()
            .unregister_connection(&connection_id);
        if let Err(error) = &outcome {
            connection.report(error).await;
        }
        let _ = send(&mut connection.socket, Message::Close(None)).await;
        outcome
    }

    async fn serve(&mut self) -> Result<(), ConnectionError> {
        let hello = self.await_hello().await?;
        let (notices, _forwarders) = self.accept(hello).await?;
        self.pump(notices).await
    }

    /// Reads the mandatory first frame; anything else ends the connection.
    async fn await_hello(&mut self) -> Result<Hello, ConnectionError> {
        loop {
            let message = tokio::time::timeout(IDLE_TIMEOUT, self.socket.recv())
                .await
                .map_err(|_| ConnectionError::Idle)?
                .ok_or(ConnectionError::Closed)?
                .map_err(|_| ConnectionError::Closed)?;
            let Some(text) = self.text_of(message)? else {
                continue;
            };
            return match ClientMessage::parse(&text) {
                Ok(ClientMessage::Hello(hello)) if hello.protocol == PROTOCOL => Ok(hello),
                Ok(ClientMessage::Hello(_)) => Err(ConnectionError::UnsupportedProtocol),
                _ => Err(ConnectionError::ExpectedHello),
            };
        }
    }

    /// Validates subscriptions, sends `welcome`, then replays each stream.
    async fn accept(
        &mut self,
        hello: Hello,
    ) -> Result<(mpsc::Receiver<(usize, StreamNotice)>, Vec<ForwarderGuard>), ConnectionError> {
        if hello.subscriptions.len() > MAX_SUBSCRIPTIONS {
            return Err(ConnectionError::TooManySubscriptions);
        }
        let mut states = Vec::with_capacity(hello.subscriptions.len());
        for requested in hello.subscriptions {
            let match_id =
                parse_match_stream(&requested.stream).ok_or(ConnectionError::UnknownStream)?;
            if self
                .subscriptions
                .iter()
                .any(|held| held.stream == requested.stream)
            {
                return Err(ConnectionError::DuplicateStream);
            }
            let page = self
                .state
                .application()
                .match_events(&self.user_id, &match_id, requested.after_seq)
                .map_err(|_| ConnectionError::ForbiddenStream)?;
            states.push((page.version(), page.latest_sequence()));
            self.subscriptions.push(Subscription {
                stream: requested.stream,
                match_id,
                cursor: requested.after_seq,
                presence: Vec::new(),
                wants_view: requested.view_patches,
                view: None,
            });
        }

        let streams = self
            .subscriptions
            .iter()
            .zip(&states)
            .map(|(subscription, (version, event_seq))| StreamState {
                stream: &subscription.stream,
                version: *version,
                event_seq: *event_seq,
            })
            .collect();
        let welcome = serde_json::to_string(&ServerMessage::welcome(
            &self.connection_id,
            HEARTBEAT_INTERVAL.as_secs(),
            streams,
        ))
        .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(welcome)).await?;

        let (sender, receiver) = mpsc::channel(MAX_SUBSCRIPTIONS * 4);
        let forwarders = self
            .subscriptions
            .iter()
            .enumerate()
            .map(|(index, subscription)| {
                ForwarderGuard::spawn(
                    self.state.realtime().subscribe(&subscription.stream),
                    sender.clone(),
                    index,
                )
            })
            .collect();
        // Listed as online only after the forwarders exist, so the wake-up this
        // join publishes reaches this connection too.
        self.presence = self
            .subscriptions
            .iter()
            .map(|subscription| {
                self.state
                    .realtime()
                    .join(&subscription.stream, &self.connection_id, &self.user_id)
            })
            .collect();
        for index in 0..self.subscriptions.len() {
            self.refresh(index).await?;
        }
        Ok((receiver, forwarders))
    }

    async fn pump(
        &mut self,
        mut notices: mpsc::Receiver<(usize, StreamNotice)>,
    ) -> Result<(), ConnectionError> {
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.tick().await;
        let mut window = RateWindow::new();
        loop {
            tokio::select! {
                message = tokio::time::timeout(IDLE_TIMEOUT, self.socket.recv()) => {
                    let message = message
                        .map_err(|_| ConnectionError::Idle)?
                        .ok_or(ConnectionError::Closed)?
                        .map_err(|_| ConnectionError::Closed)?;
                    if !window.allow() {
                        return Err(ConnectionError::RateLimited);
                    }
                    if let Some(text) = self.text_of(message)? {
                        self.dispatch(&text).await?;
                    }
                }
                notice = notices.recv() => {
                    let (index, notice) = notice.ok_or(ConnectionError::Internal)?;
                    match notice {
                        StreamNotice::Changed { .. } => self.refresh(index).await?,
                        StreamNotice::Chat { seat, message_type, content } => {
                            self.send_chat(index, seat, message_type, &content).await?;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    send(&mut self.socket, Message::Ping(Vec::new().into())).await?;
                }
                _ = Self::wait_for_revocation(&self.close_signal) => {
                    return Ok(());
                }
            }
        }
    }

    /// Resolves when the user's sessions are revoked (login from another client).
    async fn wait_for_revocation(signal: &Option<Arc<Notify>>) {
        match signal {
            Some(signal) => signal.notified().await,
            None => std::future::pending().await,
        }
    }

    async fn dispatch(&mut self, text: &str) -> Result<(), ConnectionError> {
        match ClientMessage::parse(text) {
            Ok(ClientMessage::Ping) => self.pong().await,
            Ok(ClientMessage::Command(command)) => self.submit(command).await,
            Ok(ClientMessage::Chat(chat)) => self.chat(chat).await,
            Ok(ClientMessage::Resync(request)) => self.resync(request).await,
            Ok(ClientMessage::Hello(_)) => Err(ConnectionError::DuplicateHello),
            Err(MessageError::UnknownKind) => {
                self.send_error(
                    None,
                    "request.unknown_kind",
                    "message kind is not supported",
                    false,
                )
                .await
            }
            Err(MessageError::Malformed { command_id }) => {
                self.send_error(
                    command_id.as_deref(),
                    "request.invalid_json",
                    "message is not a valid envelope",
                    false,
                )
                .await
            }
        }
    }

    async fn submit(&mut self, command: ClientCommand) -> Result<(), ConnectionError> {
        let Some(index) = self
            .subscriptions
            .iter()
            .position(|held| held.stream == command.stream)
        else {
            return self
                .send_error(
                    Some(&command.command_id),
                    "request.unknown_stream",
                    "connection is not subscribed to this stream",
                    false,
                )
                .await;
        };
        let match_id = self.subscriptions[index].match_id.clone();
        let before = self.subscriptions[index].cursor;
        let result = super::matches::apply_command(
            &self.state,
            &self.user_id,
            &match_id,
            SubmitGameCommand {
                expected_version: command.expected_version,
                command: command.command.into(),
            },
        )
        .await;
        match result {
            Ok(view) => {
                let applied = serde_json::to_string(&ServerMessage::applied(
                    &command.command_id,
                    view.version(),
                    ((before + 1)..=view.event_sequence()).collect(),
                ))
                .map_err(|_| ConnectionError::Internal)?;
                send(&mut self.socket, Message::text(applied)).await?;
                self.refresh(index).await
            }
            Err(error) => {
                let (code, message, retryable) =
                    (error.code(), error.message().to_owned(), error.retryable());
                self.send_error(Some(&command.command_id), code, &message, retryable)
                    .await
            }
        }
    }

    /// Broadcasts a short-lived table chat message to every live observer.
    ///
    /// The client never supplies the seat. It is resolved from the authenticated
    /// subscription here, so a player cannot impersonate another seat.
    async fn chat(&mut self, chat: ClientChat) -> Result<(), ConnectionError> {
        let Some((stream, match_id)) = self
            .subscriptions
            .iter()
            .find(|held| held.stream == chat.stream)
            .map(|held| (held.stream.clone(), held.match_id.clone()))
        else {
            return self
                .send_error(
                    None,
                    "request.unknown_stream",
                    "connection is not subscribed to this stream",
                    false,
                )
                .await;
        };

        let content = match chat.message_type {
            ChatMessageType::Text => {
                let content = chat.content.trim();
                if content.is_empty() || content.chars().count() > MAX_CHAT_TEXT_CHARS {
                    return self
                        .send_error(
                            None,
                            "request.invalid_chat",
                            "chat text must contain between 1 and 200 characters",
                            false,
                        )
                        .await;
                }
                content.to_owned()
            }
            ChatMessageType::Emoji => {
                let content = chat.content.as_str();
                if content.len() > MAX_CHAT_EMOJI_BYTES
                    || !content.starts_with('/')
                    || content.starts_with("//")
                    || content.chars().any(char::is_control)
                {
                    return self
                        .send_error(
                            None,
                            "request.invalid_chat",
                            "chat emoji must be a local asset path",
                            false,
                        )
                        .await;
                }
                content.to_owned()
            }
        };

        let projection = self
            .state
            .application()
            .match_projection(&self.user_id, &match_id)
            .map_err(|_| ConnectionError::ForbiddenStream)?;
        let seat = projection
            .seated()
            .into_iter()
            .find_map(|(seat, user_id)| (user_id == &self.user_id).then_some(seat))
            .ok_or(ConnectionError::ForbiddenStream)?;

        self.state.realtime().publish(
            &stream,
            StreamNotice::Chat {
                seat,
                message_type: chat.message_type,
                content,
            },
        );
        Ok(())
    }

    async fn send_chat(
        &mut self,
        index: usize,
        seat: u8,
        message_type: ChatMessageType,
        content: &str,
    ) -> Result<(), ConnectionError> {
        let stream = self
            .subscriptions
            .get(index)
            .ok_or(ConnectionError::Internal)?
            .stream
            .clone();
        let frame =
            serde_json::to_string(&ServerMessage::chat(&stream, seat, message_type, content))
                .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(frame)).await
    }

    /// Carries server time so clients can align action countdowns.
    async fn pong(&mut self) -> Result<(), ConnectionError> {
        let server_time = server_time()?;
        let latest_seq = self
            .subscriptions
            .first()
            .map_or(0, |subscription| subscription.cursor);
        let pong = serde_json::to_string(&ServerMessage::Pong {
            server_time: &server_time,
            latest_seq,
        })
        .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(pong)).await
    }

    /// Delivers everything a wake-up may have changed.
    ///
    /// 帧的顺序是固定的：视图（快照或补丁）→ `clock` → `presence`。倒计时和
    /// 在线状态都是视图的附属信息，必须排在视图之后。观察者视图只读一次，
    /// 三段共用，避免同一次唤醒里读出两份时刻不同的数据。
    async fn refresh(&mut self, index: usize) -> Result<(), ConnectionError> {
        let match_id = {
            let subscription = self
                .subscriptions
                .get(index)
                .ok_or(ConnectionError::Internal)?;
            subscription.match_id.clone()
        };
        if !self.subscriptions[index].wants_view {
            self.flush(index).await?;
        }
        let view = self
            .state
            .application()
            .match_projection(&self.user_id, &match_id)
            .map_err(|_| ConnectionError::ForbiddenStream)?;
        if self.subscriptions[index].wants_view {
            // 事件帧不发，但游标照常推进：重连时还要靠它报 `after_seq`。
            self.subscriptions[index].cursor = view.event_sequence();
            self.send_view(index, &view).await?;
        }
        self.send_status(index, &view).await
    }

    /// 首帧发整份视图，之后只发它和上一份之间的差；没差就一帧都不发。
    async fn send_view(
        &mut self,
        index: usize,
        view: &MatchProjection,
    ) -> Result<(), ConnectionError> {
        let next = serde_json::to_value(MatchViewResponse::from_projection(
            view,
            self.state.now_ms(),
            self.state.application(),
        ))
        .map_err(|_| ConnectionError::Internal)?;
        let stream = self.subscriptions[index].stream.clone();
        let frame = match self.subscriptions[index].view.as_ref() {
            Some(previous) => {
                let Some(ops) = viewdelta::diff(previous, &next) else {
                    return Ok(());
                };
                // 底子是客户端手上那份视图自己写着的版本号，两边比的是同一个数。
                let base_version = previous["version"].as_u64().unwrap_or_default();
                serde_json::to_string(&ServerMessage::view_patch(
                    &stream,
                    base_version,
                    view.version(),
                    &ops,
                ))
            }
            None => serde_json::to_string(&ServerMessage::view_snapshot(
                &stream,
                view.version(),
                &next,
            )),
        }
        .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(frame)).await?;
        // 先发帧后记账：发失败就还留着旧的那份，下次仍从它算差，不会把补丁
        // 建立在一份对方根本没收到的视图上。
        self.subscriptions[index].view = Some(next);
        Ok(())
    }

    /// 丢掉记着的上一份视图，下一次刷新因此重发一份完整快照。
    async fn resync(&mut self, request: Resync) -> Result<(), ConnectionError> {
        let Some(index) = self
            .subscriptions
            .iter()
            .position(|held| held.stream == request.stream)
        else {
            return self
                .send_error(
                    None,
                    "request.unknown_stream",
                    "connection is not subscribed to this stream",
                    false,
                )
                .await;
        };
        self.subscriptions[index].view = None;
        self.refresh(index).await
    }

    /// Sends the countdown, then presence when it differs from the last one.
    ///
    /// One observer read serves both frames, and deduplicating presence keeps
    /// a wake-up that changed nothing silent.
    async fn send_status(
        &mut self,
        index: usize,
        view: &MatchProjection,
    ) -> Result<(), ConnectionError> {
        let stream = self
            .subscriptions
            .get(index)
            .ok_or(ConnectionError::Internal)?
            .stream
            .clone();

        let seats = SeatCountdown::snapshot(view.clocks(), self.state.now_ms());
        let server_time = server_time()?;
        let clock = serde_json::to_string(&ServerMessage::clock(
            &stream,
            view.version(),
            &server_time,
            &seats,
        ))
        .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(clock)).await?;

        let online = self.state.realtime().online_users(&stream);
        let presence: Vec<SeatPresence> = view
            .seated()
            .into_iter()
            .map(|(seat, user_id)| SeatPresence {
                seat,
                online: online.contains(user_id),
            })
            .collect();
        if self.subscriptions[index].presence == presence {
            return Ok(());
        }
        let frame = serde_json::to_string(&ServerMessage::presence(&stream, &presence))
            .map_err(|_| ConnectionError::Internal)?;
        self.subscriptions[index].presence = presence;
        send(&mut self.socket, Message::text(frame)).await
    }

    /// Pulls redacted events by cursor until the stream is drained.
    async fn flush(&mut self, index: usize) -> Result<(), ConnectionError> {
        loop {
            let subscription = self
                .subscriptions
                .get(index)
                .ok_or(ConnectionError::Internal)?;
            let page: MatchEventPage = self
                .state
                .application()
                .match_events(&self.user_id, &subscription.match_id, subscription.cursor)
                .map_err(|_| ConnectionError::ForbiddenStream)?;
            if page.events().is_empty() {
                return Ok(());
            }
            let mut frames = Vec::with_capacity(page.events().len());
            let mut cursor = subscription.cursor;
            for event in page.events() {
                frames.push(
                    serde_json::to_string(&ServerMessage::event(
                        &subscription.stream,
                        page.version(),
                        event,
                    ))
                    .map_err(|_| ConnectionError::Internal)?,
                );
                cursor = event.sequence();
            }
            for frame in frames {
                send(&mut self.socket, Message::text(frame)).await?;
            }
            self.subscriptions[index].cursor = cursor;
            if cursor >= page.latest_sequence() {
                return Ok(());
            }
        }
    }

    async fn send_error(
        &mut self,
        command_id: Option<&str>,
        code: &str,
        message: &str,
        retryable: bool,
    ) -> Result<(), ConnectionError> {
        let frame =
            serde_json::to_string(&ServerMessage::error(command_id, code, message, retryable))
                .map_err(|_| ConnectionError::Internal)?;
        send(&mut self.socket, Message::text(frame)).await
    }

    /// Reports the terminal condition before the socket closes.
    async fn report(&mut self, error: &ConnectionError) {
        let Some((code, message)) = error.envelope() else {
            return;
        };
        let Ok(frame) = serde_json::to_string(&ServerMessage::error(None, code, message, false))
        else {
            return;
        };
        let _ = send(&mut self.socket, Message::text(frame)).await;
    }

    /// Keeps only text frames; control frames are handled by the transport.
    fn text_of(&self, message: Message) -> Result<Option<Utf8Bytes>, ConnectionError> {
        match message {
            Message::Text(text) if text.len() > MAX_MESSAGE_BYTES => {
                Err(ConnectionError::MessageTooLarge)
            }
            Message::Text(text) => Ok(Some(text)),
            Message::Binary(_) => Err(ConnectionError::BinaryFrame),
            Message::Close(_) => Err(ConnectionError::Closed),
            Message::Ping(_) | Message::Pong(_) => Ok(None),
        }
    }
}

/// Wall-clock time clients use to correct countdown drift.
fn server_time() -> Result<String, ConnectionError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ConnectionError::Internal)
}

async fn send(socket: &mut WebSocket, message: Message) -> Result<(), ConnectionError> {
    tokio::time::timeout(SEND_TIMEOUT, socket.send(message))
        .await
        .map_err(|_| ConnectionError::SlowConsumer)?
        .map_err(|_| ConnectionError::Closed)
}

/// Forwards stream notices into the connection's single wake-up channel.
struct ForwarderGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl ForwarderGuard {
    fn spawn(
        mut notices: broadcast::Receiver<StreamNotice>,
        sender: mpsc::Sender<(usize, StreamNotice)>,
        index: usize,
    ) -> Self {
        Self {
            handle: tokio::spawn(async move {
                loop {
                    match notices.recv().await {
                        // Lagging only costs a wake-up; the cursor pull catches up.
                        Ok(notice) => {
                            if sender.send((index, notice)).await.is_err() {
                                return;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Persistent state can be recovered by refreshing.
                            // Ephemeral chat messages that fell out of the buffer expire anyway.
                            let refresh = StreamNotice::Changed {
                                version: 0,
                                latest_sequence: 0,
                            };
                            if sender.send((index, refresh)).await.is_err() {
                                return;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => return,
                    }
                }
            }),
        }
    }
}

impl Drop for ForwarderGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Fixed one-second window; a burst above the limit closes the connection.
struct RateWindow {
    started: tokio::time::Instant,
    count: u32,
}

impl RateWindow {
    fn new() -> Self {
        Self {
            started: tokio::time::Instant::now(),
            count: 0,
        }
    }

    fn allow(&mut self) -> bool {
        let now = tokio::time::Instant::now();
        if now.duration_since(self.started) >= Duration::from_secs(1) {
            self.started = now;
            self.count = 0;
        }
        self.count += 1;
        self.count <= MAX_MESSAGES_PER_SECOND
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionError {
    Closed,
    Idle,
    Internal,
    ExpectedHello,
    DuplicateHello,
    UnsupportedProtocol,
    UnknownStream,
    DuplicateStream,
    ForbiddenStream,
    TooManySubscriptions,
    MessageTooLarge,
    BinaryFrame,
    RateLimited,
    SlowConsumer,
}

impl ConnectionError {
    /// Terminal conditions the client can act on; transport failures stay silent.
    const fn envelope(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Closed | Self::Idle | Self::SlowConsumer => None,
            Self::Internal => Some(("server.internal", "internal server error")),
            Self::ExpectedHello => Some((
                "request.expected_hello",
                "the first message must be a hello envelope",
            )),
            Self::DuplicateHello => Some((
                "request.expected_hello",
                "hello is only accepted once per connection",
            )),
            Self::UnsupportedProtocol => Some((
                "request.unsupported_protocol",
                "client protocol version is not supported",
            )),
            Self::UnknownStream => Some(("request.unknown_stream", "stream name is not supported")),
            Self::DuplicateStream => Some((
                "request.unknown_stream",
                "the same stream cannot be subscribed twice",
            )),
            Self::ForbiddenStream => Some((
                "auth.forbidden_stream",
                "user is not an observer of this stream",
            )),
            Self::TooManySubscriptions => Some((
                "request.too_many_subscriptions",
                "connection exceeds the subscription limit",
            )),
            Self::MessageTooLarge => Some((
                "request.message_too_large",
                "message exceeds the size limit",
            )),
            Self::BinaryFrame => Some(("request.invalid_json", "binary frames are not accepted")),
            Self::RateLimited => Some(("request.rate_limited", "client message rate is too high")),
        }
    }
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.envelope() {
            Some((code, _)) => formatter.write_str(code),
            None => write!(formatter, "{self:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use futures_util::{SinkExt, StreamExt};
    use mahjong_core::MatchId;
    use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
    use mamahjong_application::{
        CreateRoom, GameCommand, RegisterUser, RoomRuleSelection, RoomVisibility, SubmitGameCommand,
    };
    use serde_json::{Value, json};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::tungstenite::{Error as WsError, Message as WsMessage};
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
    use tower::ServiceExt;

    use super::message::PROTOCOL;
    use super::{MAX_MESSAGE_BYTES, MAX_MESSAGES_PER_SECOND, RateWindow, match_stream};
    use crate::clock::sweep;
    use crate::{AppState, build_router};

    type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

    /// Frames never take this long on a loopback socket; a stall is a failure.
    const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

    /// A started three-player match served over a real TCP port.
    struct Table {
        address: SocketAddr,
        router: Router,
        state: AppState,
        match_id: MatchId,
        /// The three seated players, then one registered outsider.
        tokens: Vec<String>,
    }

    async fn sanma_table(suffix: &str) -> Table {
        let state = AppState::new();
        let mut users = Vec::new();
        let mut tokens = Vec::new();
        for index in 0..4 {
            let (user, session) = state
                .application()
                .register(RegisterUser {
                    login_name: format!("ws_{suffix}_{index}"),
                    password: "correct horse battery staple".to_owned(),
                    nickname: format!("玩家{index}"),
                })
                .expect("register");
            tokens.push(session.token().to_owned());
            users.push(user);
        }
        let mut room = state
            .application()
            .create_room(
                users[0].id(),
                CreateRoom {
                    name: "实时三麻".to_owned(),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: RiichiVariant::Sanma,
                        request: RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        for user in &users[1..3] {
            room = state
                .application()
                .join_room(user.id(), room.id(), room.version())
                .expect("join");
        }
        for user in &users[..3] {
            room = state
                .application()
                .set_ready(user.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = state
            .application()
            .start_room(users[0].id(), room.id(), room.version(), state.now_ms())
            .expect("start");
        for user in &users[..3] {
            state
                .application()
                .submit_game_command(
                    user.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::MatchAssetsReady,
                    },
                    state.now_ms(),
                )
                .expect("assets ready");
        }
        for user in &users[..3] {
            state
                .application()
                .submit_game_command(
                    user.id(),
                    &match_id,
                    SubmitGameCommand {
                        expected_version: 0,
                        command: GameCommand::ReadyForHand { hand_index: 0 },
                    },
                    state.now_ms(),
                )
                .expect("opening ready");
        }
        let mut seat_tokens = vec![String::new(); 3];
        for (user, token) in users[..3].iter().zip(tokens[..3].iter()) {
            let seat = state
                .application()
                .match_view(user.id(), &match_id)
                .expect("player match view")
                .observer_seat();
            seat_tokens[usize::from(seat.index())] = token.clone();
        }
        seat_tokens.push(tokens[3].clone());

        let router = build_router(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let served = router.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, served).await;
        });
        Table {
            address,
            router,
            state,
            match_id,
            tokens: seat_tokens,
        }
    }

    async fn http_json(
        table: &Table,
        method: Method,
        uri: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(&value).expect("encode JSON"))
            }
            None => Body::empty(),
        };
        let response = table
            .router
            .clone()
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1_048_576)
            .await
            .expect("response body");
        (
            status,
            serde_json::from_slice(&bytes).expect("JSON response"),
        )
    }

    async fn ticket(table: &Table, token: &str) -> String {
        let (status, issued) =
            http_json(table, Method::POST, "/api/v1/ws-tickets", token, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(issued["schema"], "ws_ticket.v1");
        issued["ticket"].as_str().expect("ticket").to_owned()
    }

    async fn upgrade(table: &Table, query: &str) -> Result<Socket, WsError> {
        let url = format!("ws://{}/api/v1/ws{query}", table.address);
        tokio_tungstenite::connect_async(url)
            .await
            .map(|(socket, _)| socket)
    }

    async fn connect(table: &Table, token: &str) -> Socket {
        let ticket = ticket(table, token).await;
        upgrade(table, &format!("?ticket={ticket}"))
            .await
            .expect("upgrade")
    }

    async fn send_json(socket: &mut Socket, value: &Value) {
        socket
            .send(WsMessage::text(value.to_string()))
            .await
            .expect("send");
    }

    /// Reads the next text frame; control frames belong to the transport.
    async fn next_json(socket: &mut Socket) -> Value {
        loop {
            let message = tokio::time::timeout(FRAME_TIMEOUT, socket.next())
                .await
                .expect("frame before timeout")
                .expect("stream is open")
                .expect("frame");
            match message {
                WsMessage::Text(text) => return serde_json::from_str(&text).expect("JSON frame"),
                WsMessage::Close(_) => panic!("connection closed before a text frame"),
                _ => continue,
            }
        }
    }

    /// The next frame that is not a status update.
    ///
    /// Any connection joining or leaving wakes the others, so `clock` and
    /// `presence` may arrive between a request and its reply.
    async fn next_reply(socket: &mut Socket) -> Value {
        loop {
            let frame = next_json(socket).await;
            if !matches!(frame["kind"].as_str(), Some("clock" | "presence")) {
                return frame;
            }
        }
    }

    async fn hello(socket: &mut Socket, stream: &str, after_seq: u64) -> Value {
        send_json(
            socket,
            &json!({
                "kind": "hello",
                "protocol": PROTOCOL,
                "subscriptions": [{"stream": stream, "after_seq": after_seq}]
            }),
        )
        .await;
        let welcome = next_reply(socket).await;
        assert_eq!(welcome["kind"], "welcome");
        assert_eq!(welcome["schema"], "welcome.v1");
        welcome
    }

    /// Subscribes asking for view frames instead of event frames.
    async fn hello_with_view(socket: &mut Socket, stream: &str) -> Value {
        send_json(
            socket,
            &json!({
                "kind": "hello",
                "protocol": PROTOCOL,
                "subscriptions": [{"stream": stream, "view_patches": true}]
            }),
        )
        .await;
        let welcome = next_reply(socket).await;
        assert_eq!(welcome["kind"], "welcome");
        welcome
    }

    /// Reads frames until `until` arrives, folding every view frame into `view`.
    ///
    /// 视图订阅必须像真正的客户端那样，一份不落地把补丁按顺序打进手上的视图：
    /// 漏掉一个，之后每一个补丁就都建立在错误的底子上。倒计时是按时刻算的，
    /// 光是加入房间引起的一次唤醒就能在指令回执之前插进一个补丁。
    async fn advance_view(socket: &mut Socket, view: &mut Value, until: &str) -> Value {
        loop {
            let frame = next_json(socket).await;
            match frame["kind"].as_str() {
                Some("view_snapshot") => *view = frame["view"].clone(),
                Some("view_patch") => {
                    assert_eq!(
                        frame["base_version"], view["version"],
                        "a patch arrived for a view this client does not hold"
                    );
                    *view = super::viewdelta::apply(view, &frame["ops"]);
                    assert_eq!(view["version"], frame["version"]);
                }
                Some("clock" | "presence" | "command_result" | "error") => {}
                _ => panic!("a view subscription must not receive {frame}"),
            }
            if frame["kind"] == until {
                return frame;
            }
        }
    }

    /// Collects event frames until the sequence the server promised has arrived.
    ///
    /// Status frames travel on the same socket and are skipped here; the tests
    /// that care about them read the socket directly.
    async fn drain_events(socket: &mut Socket, until: u64) -> Vec<Value> {
        let mut events: Vec<Value> = Vec::new();
        while events
            .last()
            .and_then(|event| event["seq"].as_u64())
            .unwrap_or(0)
            < until
        {
            let frame = next_json(socket).await;
            match frame["kind"].as_str() {
                Some("event") => events.push(frame),
                Some("clock" | "presence") => continue,
                _ => panic!("unexpected frame {frame}"),
            }
        }
        events
    }

    fn sequences(events: &[Value]) -> Vec<u64> {
        events
            .iter()
            .map(|event| event["seq"].as_u64().expect("seq"))
            .collect()
    }

    async fn expect_error(socket: &mut Socket, code: &str) {
        let frame = next_reply(socket).await;
        assert_eq!(frame["kind"], "error", "unexpected frame {frame}");
        assert_eq!(frame["code"], code);
    }

    #[test]
    fn stream_names_round_trip_a_match_identifier() {
        let match_id = MatchId::new();
        let stream = match_stream(&match_id);

        assert!(stream.starts_with("match_"));
        assert_eq!(super::parse_match_stream(&stream), Some(match_id));
        assert!(super::parse_match_stream("lobby").is_none());
        assert!(super::parse_match_stream("match_not-an-id").is_none());
    }

    #[test]
    fn a_burst_above_the_limit_is_rejected() {
        let mut window = RateWindow::new();
        for _ in 0..MAX_MESSAGES_PER_SECOND {
            assert!(window.allow());
        }
        assert!(!window.allow());
    }

    #[tokio::test]
    async fn players_share_a_sequence_but_only_see_their_own_tiles() {
        let table = sanma_table("play").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let mut opponent = connect(&table, &table.tokens[1]).await;

        let welcome = hello(&mut dealer, &stream, 0).await;
        let watcher_welcome = hello(&mut opponent, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");
        assert_eq!(watcher_welcome["streams"][0]["event_seq"], dealt);

        let own = drain_events(&mut dealer, dealt).await;
        let watched = drain_events(&mut opponent, dealt).await;
        assert_eq!(sequences(&own), sequences(&watched));
        assert_eq!(
            sequences(&own)[0],
            1,
            "a full replay starts at the first event"
        );

        let hand = own
            .iter()
            .find(|event| {
                event["name"] == "riichi.initial_hand_dealt" && event["payload"]["seat"] == 0
            })
            .expect("own opening hand");
        let hidden = watched
            .iter()
            .find(|event| event["seq"] == hand["seq"])
            .expect("the same event");
        let tiles = hand["payload"]["tiles"].as_array().expect("own tiles");
        assert!(hidden["payload"]["tiles"].is_null(), "tiles stay hidden");
        assert_eq!(hidden["payload"]["tile_count"], tiles.len());

        let drawn = own
            .iter()
            .rev()
            .find(|event| event["name"] == "riichi.tile_drawn" && event["payload"]["seat"] == 0)
            .expect("own draw");
        let discard = json!({
            "kind": "command",
            "command_id": "c1",
            "stream": stream,
            "expected_version": welcome["streams"][0]["version"],
            "name": "riichi.discard",
            "payload": {"tile_id": drawn["payload"]["tile"]["id"]}
        });
        send_json(&mut dealer, &discard).await;

        let applied = next_reply(&mut dealer).await;
        assert_eq!(applied["kind"], "command_result");
        assert_eq!(applied["command_id"], "c1");
        assert_eq!(applied["status"], "applied");
        let latest = applied["event_seq"]
            .as_array()
            .and_then(|sequences| sequences.last())
            .and_then(Value::as_u64)
            .expect("applied sequences");
        assert_eq!(
            latest,
            dealt + applied["event_seq"].as_array().expect("sequences").len() as u64
        );

        let played = drain_events(&mut dealer, latest).await;
        let seen = drain_events(&mut opponent, latest).await;
        assert_eq!(sequences(&played), sequences(&seen));
        let discarded = played
            .iter()
            .find(|event| event["name"] == "riichi.tile_discarded")
            .expect("discard event");
        assert_eq!(
            discarded["payload"]["tile"]["id"],
            drawn["payload"]["tile"]["id"]
        );
    }

    #[tokio::test]
    async fn a_view_subscription_gets_one_snapshot_and_then_only_differences() {
        let table = sanma_table("view_patch").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        hello_with_view(&mut dealer, &stream).await;

        let mut view = Value::Null;
        let snapshot = advance_view(&mut dealer, &mut view, "view_snapshot").await;
        assert_eq!(snapshot["schema"], "view_snapshot.v1");
        assert_eq!(snapshot["stream"], stream);
        assert_eq!(view["schema"], "match_view.v1");
        assert_eq!(view["observer_seat"], 0);

        let started = view["version"].clone();
        let tile_id = view["players"][0]["drawn_tile_id"]
            .as_u64()
            .expect("the dealer holds a drawn tile");
        send_json(
            &mut dealer,
            &json!({
                "kind": "command",
                "command_id": "c1",
                "stream": stream,
                "expected_version": started,
                "name": "riichi.discard",
                "payload": {"tile_id": tile_id}
            }),
        )
        .await;
        advance_view(&mut dealer, &mut view, "command_result").await;

        let patch = advance_view(&mut dealer, &mut view, "view_patch").await;
        assert_eq!(patch["schema"], "view_patch.v1");
        assert!(patch["version"].as_u64() > started.as_u64());
        // 每次都一样的身份字段不该再出现在线路上。
        let wire = patch["ops"].to_string();
        assert!(!wire.contains("玩家0"), "the patch repeated a nickname");
        assert!(!wire.contains("user_id"), "the patch repeated identifiers");

        // 打完之后手上少了那张牌，而这只从补丁里得出，没有再拉过整份视图。
        let (status, fetched) = http_json(
            &table,
            Method::GET,
            &format!("/api/v1/matches/{}", table.match_id),
            &table.tokens[0],
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // 倒计时按当前时刻算，两次读必然不同，比较时先摘掉。
        let mut patched = view.clone();
        let mut expected = fetched;
        patched["clocks"] = Value::Null;
        expected["clocks"] = Value::Null;
        assert_eq!(
            patched, expected,
            "the patched view must match what HTTP would have returned"
        );
    }

    #[tokio::test]
    async fn resync_resends_a_full_snapshot() {
        let table = sanma_table("view_resync").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        hello_with_view(&mut dealer, &stream).await;
        let mut view = Value::Null;
        let first = advance_view(&mut dealer, &mut view, "view_snapshot").await;

        send_json(&mut dealer, &json!({"kind": "resync", "stream": stream})).await;
        let again = advance_view(&mut dealer, &mut view, "view_snapshot").await;
        assert_eq!(again["view"]["version"], first["view"]["version"]);
        assert_eq!(again["view"], view, "a snapshot replaces the held view");
    }

    #[tokio::test]
    async fn a_ticket_opens_exactly_one_socket() {
        let table = sanma_table("ticket").await;
        let ticket = ticket(&table, &table.tokens[0]).await;
        let accepted = upgrade(&table, &format!("?ticket={ticket}")).await;
        assert!(accepted.is_ok());

        for query in [
            format!("?ticket={ticket}"),
            "?ticket=00000000".to_owned(),
            String::new(),
        ] {
            let rejected = upgrade(&table, &query).await.expect_err("rejected");
            assert!(
                matches!(rejected, WsError::Http(response) if response.status().is_client_error()),
                "an unusable ticket must not upgrade"
            );
        }
    }

    #[tokio::test]
    async fn only_match_players_may_subscribe() {
        let table = sanma_table("outsider").await;
        let stream = match_stream(&table.match_id);

        let mut outsider = connect(&table, &table.tokens[3]).await;
        send_json(
            &mut outsider,
            &json!({
                "kind": "hello",
                "protocol": PROTOCOL,
                "subscriptions": [{"stream": stream}]
            }),
        )
        .await;
        expect_error(&mut outsider, "auth.forbidden_stream").await;

        let mut player = connect(&table, &table.tokens[0]).await;
        send_json(
            &mut player,
            &json!({
                "kind": "hello",
                "protocol": PROTOCOL,
                "subscriptions": [{"stream": "lobby"}]
            }),
        )
        .await;
        expect_error(&mut player, "request.unknown_stream").await;
    }

    #[tokio::test]
    async fn table_chat_is_broadcast_to_every_player_with_a_server_owned_seat() {
        let table = sanma_table("chat").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let mut opponent = connect(&table, &table.tokens[1]).await;

        let dealer_welcome = hello(&mut dealer, &stream, 0).await;
        let opponent_welcome = hello(&mut opponent, &stream, 0).await;
        let latest = dealer_welcome["streams"][0]["event_seq"]
            .as_u64()
            .expect("seq");
        assert_eq!(opponent_welcome["streams"][0]["event_seq"], latest);
        drain_events(&mut dealer, latest).await;
        drain_events(&mut opponent, latest).await;

        send_json(
            &mut dealer,
            &json!({
                "kind": "chat",
                "stream": stream,
                "type": "text",
                "content": "  大家好  "
            }),
        )
        .await;

        for socket in [&mut dealer, &mut opponent] {
            let chat = next_reply(socket).await;
            assert_eq!(chat["kind"], "chat");
            assert_eq!(chat["schema"], "chat.v1");
            assert_eq!(chat["stream"], stream);
            assert_eq!(chat["seat"], 0, "the authenticated user owns the seat");
            assert_eq!(chat["type"], "text");
            assert_eq!(chat["content"], "大家好");
        }

        send_json(
            &mut opponent,
            &json!({
                "kind": "chat",
                "stream": stream,
                "type": "emoji",
                "content": "/game/assets/emotes/smile.png"
            }),
        )
        .await;
        for socket in [&mut dealer, &mut opponent] {
            let chat = next_reply(socket).await;
            assert_eq!(chat["kind"], "chat");
            assert_eq!(chat["seat"], 1);
            assert_eq!(chat["type"], "emoji");
            assert_eq!(chat["content"], "/game/assets/emotes/smile.png");
        }
    }

    #[tokio::test]
    async fn the_first_frame_must_be_a_supported_hello() {
        let table = sanma_table("handshake").await;

        let mut early = connect(&table, &table.tokens[0]).await;
        send_json(&mut early, &json!({"kind": "ping"})).await;
        expect_error(&mut early, "request.expected_hello").await;

        let mut ancient = connect(&table, &table.tokens[0]).await;
        send_json(
            &mut ancient,
            &json!({"kind": "hello", "protocol": "mamahjong.v0"}),
        )
        .await;
        expect_error(&mut ancient, "request.unsupported_protocol").await;
    }

    #[tokio::test]
    async fn resuming_with_a_cursor_neither_repeats_nor_skips_events() {
        let table = sanma_table("resume").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut dealer, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");
        let replay = drain_events(&mut dealer, dealt).await;
        drop(dealer);

        let drawn = replay
            .iter()
            .rev()
            .find(|event| event["name"] == "riichi.tile_drawn" && event["payload"]["seat"] == 0)
            .expect("own draw");
        let (status, view) = http_json(
            &table,
            Method::POST,
            &format!("/api/v1/matches/{}/commands", table.match_id),
            &table.tokens[0],
            Some(json!({
                "expected_version": welcome["streams"][0]["version"],
                "command": {
                    "name": "riichi.discard",
                    "payload": {"tile_id": drawn["payload"]["tile"]["id"]}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{view}");

        let mut resumed = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut resumed, &stream, dealt).await;
        let latest = welcome["streams"][0]["event_seq"].as_u64().expect("seq");
        assert!(latest > dealt, "the offline discard advanced the stream");

        let missed = drain_events(&mut resumed, latest).await;
        assert_eq!(sequences(&missed), (dealt + 1..=latest).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn oversized_and_too_frequent_frames_close_the_connection() {
        let table = sanma_table("flow").await;
        let stream = match_stream(&table.match_id);

        let mut bulky = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut bulky, &stream, 0).await;
        drain_events(
            &mut bulky,
            welcome["streams"][0]["event_seq"].as_u64().expect("seq"),
        )
        .await;
        send_json(
            &mut bulky,
            &json!({"kind": "ping", "padding": "x".repeat(MAX_MESSAGE_BYTES)}),
        )
        .await;
        expect_error(&mut bulky, "request.message_too_large").await;

        let mut noisy = connect(&table, &table.tokens[1]).await;
        let welcome = hello(&mut noisy, &stream, 0).await;
        drain_events(
            &mut noisy,
            welcome["streams"][0]["event_seq"].as_u64().expect("seq"),
        )
        .await;
        for _ in 0..=MAX_MESSAGES_PER_SECOND {
            send_json(&mut noisy, &json!({"kind": "ping"})).await;
        }
        let mut codes = Vec::new();
        for _ in 0..=MAX_MESSAGES_PER_SECOND {
            let frame = next_reply(&mut noisy).await;
            if frame["kind"] == "error" {
                codes.push(frame["code"].as_str().expect("code").to_owned());
                break;
            }
            assert_eq!(frame["kind"], "pong");
            assert!(frame["server_time"].is_string());
            assert!(frame["latest_seq"].is_u64());
        }
        assert_eq!(codes, vec!["request.rate_limited".to_owned()]);
    }

    /// Reads events that arrive after `after_seq`, stopping when a clock frame
    /// signals the end of a refresh cycle.
    async fn events_after(socket: &mut Socket, after_seq: u64) -> Vec<Value> {
        let mut events = Vec::new();
        loop {
            let frame = next_json(socket).await;
            match frame["kind"].as_str() {
                Some("event") => {
                    let seq = frame["seq"].as_u64().expect("seq");
                    if seq > after_seq {
                        events.push(frame);
                    }
                }
                Some("clock" | "presence") => {
                    if !events.is_empty() {
                        return events;
                    }
                }
                other => panic!("unexpected frame kind: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn welcome_is_followed_by_a_clock_frame() {
        let table = sanma_table("clock_on_welcome").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut dealer, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");

        drain_events(&mut dealer, dealt).await;

        // The initial refresh sent a clock frame; a second one follows the join
        // wake-up.  drain_events skips status frames, so the second clock is
        // still in the buffer.
        let clock = next_json(&mut dealer).await;
        assert_eq!(clock["kind"], "clock");
        assert_eq!(clock["schema"], "clock.v1");
        assert_eq!(clock["stream"], stream);

        let seats = clock["seats"].as_array().expect("seats array");
        assert_eq!(seats.len(), 1, "only the dealer is on the clock");
        assert!(seats[0]["remaining_ms"].as_u64().unwrap() > 0);
        assert!(seats[0]["base_ms"].as_u64().unwrap() > 0);
        assert!(seats[0]["reserve_ms"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn a_timeout_produces_the_same_events_for_every_observer() {
        let table = sanma_table("sweep_share").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let mut opponent = connect(&table, &table.tokens[1]).await;

        let welcome = hello(&mut dealer, &stream, 0).await;
        let opp_welcome = hello(&mut opponent, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");
        assert_eq!(opp_welcome["streams"][0]["event_seq"], dealt);

        drain_events(&mut dealer, dealt).await;
        drain_events(&mut opponent, dealt).await;

        table.state.advance_clock(
            mamahjong_application::BASE_THINKING_MS
                + u64::from(mamahjong_application::RESERVE_THINKING_MS),
        );
        sweep(&table.state).await;

        let dealer_events = events_after(&mut dealer, dealt).await;
        let opponent_events = events_after(&mut opponent, dealt).await;

        assert!(!dealer_events.is_empty(), "the timeout produced events");
        assert_eq!(
            sequences(&dealer_events),
            sequences(&opponent_events),
            "every observer sees the same timeout events"
        );
    }

    #[tokio::test]
    async fn remaining_time_is_continuous_across_reconnects() {
        let table = sanma_table("reconnect_clock").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut dealer, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");

        drain_events(&mut dealer, dealt).await;
        drop(dealer);

        // Drain the base time while disconnected.
        let full = mamahjong_application::BASE_THINKING_MS
            + u64::from(mamahjong_application::RESERVE_THINKING_MS);
        table
            .state
            .advance_clock(mamahjong_application::BASE_THINKING_MS);

        let mut resumed = connect(&table, &table.tokens[0]).await;
        let welcome = hello(&mut resumed, &stream, dealt).await;
        assert_eq!(
            welcome["streams"][0]["event_seq"].as_u64().expect("seq"),
            dealt,
            "no events were produced while disconnected"
        );

        // The clock frame arrives after the initial refresh.
        let clock = next_json(&mut resumed).await;
        assert_eq!(clock["kind"], "clock");
        let remaining = clock["seats"][0]["remaining_ms"]
            .as_u64()
            .expect("remaining");
        assert!(
            remaining < full,
            "clock was not reset: {remaining} equals the full {full}"
        );
        // The base time drained but the reserve is intact.
        assert!(
            clock["seats"][0]["base_ms"].as_u64().unwrap()
                < mamahjong_application::BASE_THINKING_MS,
            "base time drained while disconnected"
        );
    }

    #[tokio::test]
    async fn other_connections_learn_when_a_player_disconnects() {
        let table = sanma_table("presence_off").await;
        let stream = match_stream(&table.match_id);
        let mut dealer = connect(&table, &table.tokens[0]).await;
        let mut opponent = connect(&table, &table.tokens[1]).await;

        let welcome = hello(&mut dealer, &stream, 0).await;
        let dealt = welcome["streams"][0]["event_seq"].as_u64().expect("seq");
        hello(&mut opponent, &stream, 0).await;

        drain_events(&mut dealer, dealt).await;
        drain_events(&mut opponent, dealt).await;

        drop(dealer);

        // The opponent's pump must receive the wake-up from the dealer's
        // PresenceGuard drop and send a presence frame.
        let mut found_offline = false;
        for _ in 0..20 {
            let frame = next_json(&mut opponent).await;
            if frame["kind"] == "presence" {
                let seats = frame["seats"].as_array().expect("seats");
                if seats
                    .iter()
                    .any(|seat| seat["seat"].as_u64() == Some(0) && seat["online"] == false)
                {
                    found_offline = true;
                    break;
                }
            }
        }
        assert!(
            found_offline,
            "the opponent must receive a presence frame marking seat 0 as offline"
        );
    }
}
