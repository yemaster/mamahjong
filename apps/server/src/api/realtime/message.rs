use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::matches::CommandPayload;
use crate::clock::SeatCountdown;

pub(crate) const PROTOCOL: &str = "mamahjong.v1";

/// Bounds the work a single frame can cause before it is parsed.
pub(crate) const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Streams one connection may follow at once.
pub(crate) const MAX_SUBSCRIPTIONS: usize = 4;

#[derive(Debug)]
pub(super) enum ClientMessage {
    Hello(Hello),
    Command(ClientCommand),
    Resync(Resync),
    Ping,
}

impl ClientMessage {
    /// Dispatches on `kind` first so each envelope keeps its own strict shape.
    ///
    /// 一条指令帧读不出来时，仍然要把它的 `command_id` 带上：客户端只认得自己
    /// 发出去的那条指令的回执，报错里少了这个编号，它就只能当作什么都没发生，
    /// 玩家看到的是「按钮点了没反应」。
    pub(super) fn parse(text: &str) -> Result<Self, MessageError> {
        let frame: Frame = serde_json::from_str(text).map_err(|_| MessageError::malformed(None))?;
        match frame.kind.as_str() {
            "hello" => serde_json::from_str(text)
                .map(Self::Hello)
                .map_err(|_| MessageError::malformed(None)),
            "command" => serde_json::from_str(text)
                .map(Self::Command)
                .map_err(|_| MessageError::malformed(frame.command_id)),
            "resync" => serde_json::from_str(text)
                .map(Self::Resync)
                .map_err(|_| MessageError::malformed(None)),
            "ping" => Ok(Self::Ping),
            _ => Err(MessageError::UnknownKind),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MessageError {
    Malformed { command_id: Option<String> },
    UnknownKind,
}

impl MessageError {
    const fn malformed(command_id: Option<String>) -> Self {
        Self::Malformed { command_id }
    }
}

#[derive(Deserialize)]
struct Frame {
    kind: String,
    /// 只在指令帧上有；宽松解析，坏帧也要尽量把它捞出来。
    #[serde(default)]
    command_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Hello {
    pub(super) protocol: String,
    #[serde(default)]
    pub(super) subscriptions: Vec<Subscription>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Subscription {
    pub(super) stream: String,
    #[serde(default)]
    pub(super) after_seq: u64,
    /// 声明后改为收视图快照与补丁，`event` 帧对这条订阅不再发送。
    ///
    /// 缺省关闭，老客户端行为完全不变。
    #[serde(default)]
    pub(super) view_patches: bool,
}

/// 客户端手上的视图和补丁的底子对不上时，沿同一条连接请求重发快照。
#[derive(Debug, Deserialize)]
pub(super) struct Resync {
    pub(super) stream: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ClientCommand {
    pub(super) command_id: String,
    pub(super) stream: String,
    pub(super) expected_version: u64,
    #[serde(flatten)]
    pub(super) command: CommandPayload,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ServerMessage<'a> {
    Welcome {
        schema: &'static str,
        connection_id: &'a str,
        protocol: &'static str,
        heartbeat_interval: u64,
        streams: Vec<StreamState<'a>>,
    },
    Event {
        schema: &'static str,
        stream: &'a str,
        seq: u64,
        version: u64,
        hand_index: u32,
        name: &'static str,
        payload_schema: u8,
        payload: &'a Value,
    },
    /// 整份观察者视图；只在订阅建立、重连和重同步时发。
    ViewSnapshot {
        schema: &'static str,
        stream: &'a str,
        version: u64,
        view: &'a Value,
    },
    /// 相对上一份已发出视图的差；`base_version` 说明它打在哪一份上。
    ViewPatch {
        schema: &'static str,
        stream: &'a str,
        base_version: u64,
        version: u64,
        ops: &'a Value,
    },
    CommandResult {
        schema: &'static str,
        command_id: &'a str,
        status: &'static str,
        version: u64,
        event_seq: Vec<u64>,
    },
    Error {
        schema: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        command_id: Option<&'a str>,
        code: &'a str,
        message: &'a str,
        retryable: bool,
    },
    Clock {
        schema: &'static str,
        stream: &'a str,
        version: u64,
        server_time: &'a str,
        seats: &'a [SeatCountdown],
    },
    Presence {
        schema: &'static str,
        stream: &'a str,
        seats: &'a [SeatPresence],
    },
    Pong {
        server_time: &'a str,
        latest_seq: u64,
    },
}

#[derive(Debug, Serialize)]
pub(super) struct StreamState<'a> {
    pub(super) stream: &'a str,
    pub(super) version: u64,
    pub(super) event_seq: u64,
}

/// Whether the seat's player currently holds a connection to this stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SeatPresence {
    pub(super) seat: u8,
    pub(super) online: bool,
}

impl<'a> ServerMessage<'a> {
    pub(super) fn welcome(
        connection_id: &'a str,
        heartbeat_interval: u64,
        streams: Vec<StreamState<'a>>,
    ) -> Self {
        Self::Welcome {
            schema: "welcome.v1",
            connection_id,
            protocol: PROTOCOL,
            heartbeat_interval,
            streams,
        }
    }

    pub(super) fn event(
        stream: &'a str,
        version: u64,
        event: &'a mamahjong_application::MatchEvent,
    ) -> Self {
        Self::Event {
            schema: "event.v1",
            stream,
            seq: event.sequence(),
            version,
            hand_index: event.hand_index(),
            name: event.name(),
            payload_schema: event.event_version(),
            payload: event.payload(),
        }
    }

    pub(super) fn clock(
        stream: &'a str,
        version: u64,
        server_time: &'a str,
        seats: &'a [SeatCountdown],
    ) -> Self {
        Self::Clock {
            schema: "clock.v1",
            stream,
            version,
            server_time,
            seats,
        }
    }

    pub(super) fn view_snapshot(stream: &'a str, version: u64, view: &'a Value) -> Self {
        Self::ViewSnapshot {
            schema: "view_snapshot.v1",
            stream,
            version,
            view,
        }
    }

    pub(super) fn view_patch(
        stream: &'a str,
        base_version: u64,
        version: u64,
        ops: &'a Value,
    ) -> Self {
        Self::ViewPatch {
            schema: "view_patch.v1",
            stream,
            base_version,
            version,
            ops,
        }
    }

    pub(super) fn presence(stream: &'a str, seats: &'a [SeatPresence]) -> Self {
        Self::Presence {
            schema: "presence.v1",
            stream,
            seats,
        }
    }

    pub(super) fn applied(command_id: &'a str, version: u64, event_seq: Vec<u64>) -> Self {
        Self::CommandResult {
            schema: "command_result.v1",
            command_id,
            status: "applied",
            version,
            event_seq,
        }
    }

    pub(super) fn error(
        command_id: Option<&'a str>,
        code: &'a str,
        message: &'a str,
        retryable: bool,
    ) -> Self {
        Self::Error {
            schema: "error.v1",
            command_id,
            code,
            message,
            retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ClientMessage, MessageError, ServerMessage, StreamState};

    #[test]
    fn hello_defaults_to_a_full_replay() {
        let ClientMessage::Hello(hello) = ClientMessage::parse(
            r#"{"kind":"hello","protocol":"mamahjong.v1","subscriptions":[{"stream":"match_a"}]}"#,
        )
        .expect("hello") else {
            panic!("expected hello");
        };

        assert_eq!(hello.protocol, "mamahjong.v1");
        assert_eq!(hello.subscriptions[0].stream, "match_a");
        assert_eq!(hello.subscriptions[0].after_seq, 0);
        // 没声明就还是老行为：收事件帧，不收视图帧。
        assert!(!hello.subscriptions[0].view_patches);
    }

    #[test]
    fn a_subscription_may_ask_for_view_patches() {
        let ClientMessage::Hello(hello) = ClientMessage::parse(
            r#"{"kind":"hello","protocol":"mamahjong.v1",
                "subscriptions":[{"stream":"match_a","view_patches":true}]}"#,
        )
        .expect("hello") else {
            panic!("expected hello");
        };

        assert!(hello.subscriptions[0].view_patches);
    }

    #[test]
    fn resync_names_the_stream_to_resend() {
        let ClientMessage::Resync(resync) =
            ClientMessage::parse(r#"{"kind":"resync","stream":"match_a"}"#).expect("resync")
        else {
            panic!("expected resync");
        };

        assert_eq!(resync.stream, "match_a");
    }

    #[test]
    fn commands_reuse_the_http_name_and_payload_pair() {
        let ClientMessage::Command(command) = ClientMessage::parse(
            r#"{"kind":"command","schema":"command.v1","command_id":"c1","stream":"match_a",
                "expected_version":42,"name":"riichi.discard","payload":{"tile_id":37}}"#,
        )
        .expect("command") else {
            panic!("expected command");
        };

        assert_eq!(command.command_id, "c1");
        assert_eq!(command.expected_version, 42);
        assert!(matches!(
            command.command,
            crate::api::matches::CommandPayload::Discard { tile_id: 37 }
        ));
    }

    #[test]
    fn commands_without_a_payload_stay_valid() {
        let ClientMessage::Command(command) = ClientMessage::parse(
            r#"{"kind":"command","command_id":"c2","stream":"match_a","expected_version":1,
                "name":"riichi.tsumo"}"#,
        )
        .expect("command") else {
            panic!("expected command");
        };

        assert!(matches!(
            command.command,
            crate::api::matches::CommandPayload::Tsumo
        ));
    }

    #[test]
    fn unknown_kinds_and_broken_frames_are_distinguished() {
        assert_eq!(
            ClientMessage::parse(r#"{"kind":"subscribe"}"#).expect_err("unknown"),
            MessageError::UnknownKind
        );
        assert_eq!(
            ClientMessage::parse("not json").expect_err("malformed"),
            MessageError::Malformed { command_id: None }
        );
        // 坏掉的指令帧仍然要认领自己的编号，客户端才知道是哪一条被拒了。
        assert_eq!(
            ClientMessage::parse(r#"{"kind":"command","command_id":"c1"}"#)
                .expect_err("incomplete"),
            MessageError::Malformed {
                command_id: Some("c1".to_owned())
            }
        );
    }

    #[test]
    fn server_envelopes_carry_their_schema() {
        let welcome = serde_json::to_value(ServerMessage::welcome(
            "conn",
            20,
            vec![StreamState {
                stream: "match_a",
                version: 3,
                event_seq: 12,
            }],
        ))
        .expect("welcome");
        assert_eq!(welcome["kind"], "welcome");
        assert_eq!(welcome["schema"], "welcome.v1");
        assert_eq!(welcome["streams"][0]["event_seq"], 12);

        let applied =
            serde_json::to_value(ServerMessage::applied("c1", 44, vec![121, 122])).expect("result");
        assert_eq!(applied["kind"], "command_result");
        assert_eq!(applied["status"], "applied");
        assert_eq!(applied["event_seq"], json!([121, 122]));

        let failure = serde_json::to_value(ServerMessage::error(
            None,
            "game.stale_version",
            "state has advanced",
            true,
        ))
        .expect("error");
        assert_eq!(failure["kind"], "error");
        assert_eq!(failure["schema"], "error.v1");
        assert!(failure.get("command_id").is_none());
    }
}
