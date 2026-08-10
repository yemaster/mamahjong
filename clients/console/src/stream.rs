use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;

const PROTOCOL: &str = "mamahjong.v1";

/// Countdown data extracted from a `clock.v1` frame.
#[derive(Clone, Debug, Default)]
pub struct SeatCountdown {
    pub seat: u8,
    pub remaining_ms: u64,
    pub base_ms: u64,
    #[allow(dead_code)]
    pub reserve_ms: u64,
}

/// Presence data extracted from a `presence.v1` frame.
#[derive(Clone, Debug)]
pub struct SeatPresence {
    #[allow(dead_code)]
    pub seat: u8,
    pub online: bool,
}

/// Events the stream task sends to the app.
#[derive(Debug)]
pub enum StreamEvent {
    /// One or more match events arrived; the app should refresh the view.
    EventsArrived,
    /// Updated clock countdowns.
    Clock { seats: Vec<SeatCountdown> },
    /// Presence changed.
    Presence { seats: Vec<SeatPresence> },
    /// Connection lost; fall back to HTTP polling.
    Disconnected,
    /// Connection restored; `after_seq` is the last event this connection
    /// resumes from so the next reconnect starts at the right cursor.
    Reconnected { after_seq: u64 },
}

/// A command to send over the WebSocket.
#[derive(Debug)]
pub(super) struct WsCommand {
    pub(super) json: String,
}

/// Manages a WebSocket connection to a match stream.
///
/// When connected, game commands travel over the socket and events drive
/// view refreshes.  When disconnected, it signals the app to fall back to
/// HTTP polling and reconnects with exponential backoff.
pub struct MatchStream {
    event_rx: mpsc::UnboundedReceiver<StreamEvent>,
    command_tx: mpsc::UnboundedSender<WsCommand>,
    connected: bool,
    last_seq: u64,
}

impl MatchStream {
    /// Creates the stream task and returns a handle the app can poll.
    pub fn connect(base_url: String, token: String, match_id: String, after_seq: u64) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        tokio::spawn(run_stream(
            base_url, token, match_id, after_seq, event_tx, command_rx,
        ));

        Self {
            event_rx,
            command_tx,
            connected: true,
            last_seq: after_seq,
        }
    }

    /// Drains all available events without blocking.
    pub fn drain(&mut self) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            match &event {
                StreamEvent::Disconnected => self.connected = false,
                StreamEvent::Reconnected { after_seq } => {
                    self.last_seq = *after_seq;
                    self.connected = true;
                }
                StreamEvent::EventsArrived => {
                    // last_seq is updated when we refresh the view via HTTP.
                }
                _ => {}
            }
            events.push(event);
        }
        events
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    #[must_use]
    pub const fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Call after a successful HTTP game view refresh so the next reconnect
    /// uses the right cursor.
    pub fn set_last_seq(&mut self, seq: u64) {
        if seq > self.last_seq {
            self.last_seq = seq;
        }
    }

    /// Sends a game command over the socket.  Silently dropped when the
    /// channel is full (the app falls back to HTTP in that case).
    pub fn send_command(&self, json: String) {
        let _ = self.command_tx.send(WsCommand { json });
    }
}

#[derive(Deserialize)]
struct TicketResponse {
    ticket: String,
    #[allow(dead_code)]
    expires_in: u64,
}

/// Fetches a one-shot WebSocket ticket from the server.
async fn fetch_ticket(http: &reqwest::Client, base_url: &str, token: &str) -> Option<String> {
    let response = http
        .post(format!("{base_url}/api/v1/ws-tickets"))
        .bearer_auth(token)
        .send()
        .await
        .ok()?;
    let body: TicketResponse = response.json().await.ok()?;
    Some(body.ticket)
}

/// Converts an HTTP base URL into the WebSocket endpoint.
fn ws_url(base_url: &str, ticket: &str) -> String {
    let host = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    format!("ws://{host}/api/v1/ws?ticket={ticket}")
}

/// One reconnection attempt; returns the received socket and the welcome frame.
async fn try_connect(
    http: &reqwest::Client,
    base_url: &str,
    token: &str,
    stream: &str,
    after_seq: u64,
) -> Option<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Value,
)> {
    let ticket = fetch_ticket(http, base_url, token).await?;
    let url = ws_url(base_url, &ticket);
    let (mut socket, _) = connect_async(&url).await.ok()?;

    let hello = serde_json::json!({
        "kind": "hello",
        "protocol": PROTOCOL,
        "subscriptions": [{"stream": stream, "after_seq": after_seq}]
    });
    socket
        .send(WsMessage::Text(hello.to_string().into()))
        .await
        .ok()?;

    let welcome = read_json(&mut socket).await?;
    if welcome["kind"] != "welcome" {
        return None;
    }
    Some((socket, welcome))
}

/// Reads one JSON text frame, skipping control frames.
async fn read_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<Value> {
    loop {
        let message = socket.next().await?.ok()?;
        match message {
            WsMessage::Text(text) => return serde_json::from_str(&text).ok(),
            WsMessage::Close(_) => return None,
            _ => continue,
        }
    }
}

fn parse_clock(frame: &Value) -> Option<Vec<SeatCountdown>> {
    let seats = frame["seats"].as_array()?;
    let mut result = Vec::with_capacity(seats.len());
    for seat in seats {
        result.push(SeatCountdown {
            seat: seat["seat"].as_u64()? as u8,
            remaining_ms: seat["remaining_ms"].as_u64()?,
            base_ms: seat["base_ms"].as_u64()?,
            reserve_ms: seat["reserve_ms"].as_u64()?,
        });
    }
    Some(result)
}

fn parse_presence(frame: &Value) -> Option<Vec<SeatPresence>> {
    let seats = frame["seats"].as_array()?;
    let mut result = Vec::with_capacity(seats.len());
    for seat in seats {
        result.push(SeatPresence {
            seat: seat["seat"].as_u64()? as u8,
            online: seat["online"].as_bool()?,
        });
    }
    Some(result)
}

/// Backing task: owns the socket lifecycle, reconnects on failure.
async fn run_stream(
    base_url: String,
    token: String,
    match_id: String,
    mut after_seq: u64,
    event_tx: mpsc::UnboundedSender<StreamEvent>,
    mut command_rx: mpsc::UnboundedReceiver<WsCommand>,
) {
    let stream_name = format!("match_{match_id}");
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .expect("stream http client");
    let mut backoff = 1u64;

    loop {
        let Some((mut socket, _welcome)) =
            try_connect(&http, &base_url, &token, &stream_name, after_seq).await
        else {
            tokio::time::sleep(Duration::from_millis(500 * backoff.min(60))).await;
            backoff = (backoff * 2).min(60);
            continue;
        };
        backoff = 1;
        let _ = event_tx.send(StreamEvent::Reconnected { after_seq });

        // Main pump: read frames + forward commands.
        loop {
            tokio::select! {
                frame = read_json(&mut socket) => {
                    let Some(frame) = frame else {
                        break; // socket closed
                    };
                    match frame["kind"].as_str() {
                        Some("event") => {
                            if let Some(seq) = frame["seq"].as_u64() {
                                after_seq = seq;
                            }
                            let _ = event_tx.send(StreamEvent::EventsArrived);
                        }
                        Some("clock") => {
                            if let Some(seats) = parse_clock(&frame) {
                                let _ = event_tx.send(StreamEvent::Clock { seats });
                            }
                        }
                        Some("presence") => {
                            if let Some(seats) = parse_presence(&frame) {
                                let _ = event_tx.send(StreamEvent::Presence { seats });
                            }
                        }
                        Some("command_result") | Some("error") | Some("pong") => {}
                        _ => {}
                    }
                }
                cmd = command_rx.recv() => {
                    let Some(cmd) = cmd else {
                        return; // app dropped the sender
                    };
                    if socket
                        .send(WsMessage::Text(cmd.json.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }

        let _ = event_tx.send(StreamEvent::Disconnected);
        tokio::time::sleep(Duration::from_millis(500 * backoff.min(60))).await;
        backoff = (backoff * 2).min(60);
    }
}
