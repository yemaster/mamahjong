import { ApiError, gameApi } from "./api";
import type { WsSeatCountdown, WsSeatPresence } from "./types";

const PROTOCOL = "mamahjong.v1";

export function websocketUrl(baseUrl: string, ticket: string): string {
  const url = new URL("/api/v1/ws", baseUrl);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.searchParams.set("ticket", ticket);
  return url.toString();
}

export type StreamEvent =
  | { kind: "events_arrived" }
  | { kind: "clock"; seats: WsSeatCountdown[]; version: number }
  | { kind: "presence"; seats: WsSeatPresence[] }
  | { kind: "latency"; milliseconds: number }
  | {
      kind: "chat";
      seat: number;
      messageType: "text" | "emoji";
      content: string;
    }
  | { kind: "disconnected" }
  | { kind: "reconnected"; afterSeq: number }
  /** 整份观察者视图；订阅建立、重连和重同步时各来一份。 */
  | { kind: "view_snapshot"; view: unknown }
  /** 相对上一份视图的差；`baseVersion` 说明它该打在哪一份上。 */
  | { kind: "view_patch"; baseVersion: number; ops: unknown }
  /** 服务端拒了刚才那条指令，得让玩家看见，不能点了没反应。 */
  | { kind: "command_rejected"; code: string; message: string };

export type WsConnState = "disconnected" | "connecting" | "connected";

interface Callbacks {
  onEvent: (event: StreamEvent) => void;
  onStateChange: (state: WsConnState) => void;
}

/**
 * Manages a WebSocket connection to a match stream.
 *
 * Owns the socket lifecycle: ticket exchange, hello/welcome handshake,
 * frame dispatch, and exponential-backoff reconnection.
 * Game commands are multiplexed over the same socket.
 */
export class MatchStream {
  private ws: WebSocket | null = null;
  private state: WsConnState = "disconnected";
  private afterSeq: number;
  private commandIdCounter = 0;
  private readonly baseUrl: string;
  private readonly token: string;
  private readonly matchId: string;
  private readonly stream: string;
  private readonly callbacks: Callbacks;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private latencyTimer: ReturnType<typeof setInterval> | null = null;
  private pingSentAt: number | null = null;
  private backoff = 1;

  constructor(
    baseUrl: string,
    token: string,
    matchId: string,
    afterSeq: number,
    callbacks: Callbacks,
  ) {
    this.baseUrl = baseUrl;
    this.token = token;
    this.matchId = matchId;
    this.afterSeq = afterSeq;
    this.stream = `match_${matchId}`;
    this.callbacks = callbacks;
  }

  connect(): void {
    if (this.state === "connecting" || this.state === "connected") {
      return;
    }
    this.setState("connecting");
    this.doConnect();
  }

  disconnect(): void {
    this.clearReconnect();
    this.stopLatencyProbe();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setState("disconnected");
  }

  sendCommand(name: string, payload?: unknown, expectedVersion?: number): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    const commandId = `c${++this.commandIdCounter}`;
    const envelope: Record<string, unknown> = {
      kind: "command",
      command_id: commandId,
      stream: this.stream,
      expected_version: expectedVersion ?? 0,
      name,
    };
    if (payload !== undefined) {
      envelope.payload = payload;
    }
    this.ws.send(JSON.stringify(envelope));
    return true;
  }

  sendChat(type: "text" | "emoji", content: string): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    this.ws.send(
      JSON.stringify({
        kind: "chat",
        stream: this.stream,
        type,
        content,
      }),
    );
    return true;
  }

  /**
   * 记下视图已经推进到的事件序号。
   *
   * 视图订阅收不到 event 帧，游标只能由视图自己报；重连时靠它告诉服务端从哪
   * 接着来，免得让对方把整局的事件重新翻一遍。
   */
  noteCursor(seq: number): void {
    if (seq > this.afterSeq) {
      this.afterSeq = seq;
    }
  }

  /**
   * 手上的视图和补丁的底子对不上时，请服务端重发一份快照。
   *
   * 走同一条连接而不是退回去拉 HTTP：HTTP 响应会和连接上的补丁赛跑，拉回来的
   * 可能反而更旧，把界面推回过去。
   */
  requestResync(): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return;
    }
    this.ws.send(JSON.stringify({ kind: "resync", stream: this.stream }));
  }

  /* ── internals ──────────────────────────── */

  private async doConnect(): Promise<void> {
    try {
      const ticketResp = await gameApi.wsTicket(this.token);
      const wsUrl = websocketUrl(this.baseUrl, ticketResp.ticket);

      const ws = new WebSocket(wsUrl);
      this.ws = ws;

      ws.onopen = () => {
        ws.send(
          JSON.stringify({
            kind: "hello",
            protocol: PROTOCOL,
            subscriptions: [
              /*
                要视图快照与补丁：整份视图约 7 KB，一小局要推进上百次，重复
                搬运的绝大部分是每次都一样的昵称、头像和已经摆在桌上的牌。
                声明之后服务端不再发 event 帧，视图本身就是完整真相。
              */
              {
                stream: this.stream,
                after_seq: this.afterSeq,
                view_patches: true,
              },
            ],
          }),
        );
      };

      ws.onmessage = (event: MessageEvent<string>) => {
        try {
          const frame = JSON.parse(event.data) as { kind: string };
          switch (frame.kind) {
            case "welcome": {
              this.backoff = 1;
              this.setState("connected");
              this.startLatencyProbe();
              this.callbacks.onEvent({
                kind: "reconnected",
                afterSeq: this.afterSeq,
              });
              break;
            }
            case "event": {
              const evt = frame as { seq?: number };
              if (typeof evt.seq === "number") {
                this.afterSeq = evt.seq;
              }
              this.callbacks.onEvent({ kind: "events_arrived" });
              break;
            }
            case "view_snapshot": {
              const snapshot = frame as { view?: unknown };
              if (snapshot.view !== undefined) {
                this.callbacks.onEvent({
                  kind: "view_snapshot",
                  view: snapshot.view,
                });
              }
              break;
            }
            case "view_patch": {
              const patch = frame as {
                base_version?: number;
                ops?: unknown;
              };
              if (patch.ops !== undefined) {
                this.callbacks.onEvent({
                  kind: "view_patch",
                  baseVersion: patch.base_version ?? 0,
                  ops: patch.ops,
                });
              }
              break;
            }
            case "clock": {
              const clock = frame as {
                seats?: WsSeatCountdown[];
                version?: number;
              };
              if (clock.seats) {
                this.callbacks.onEvent({
                  kind: "clock",
                  seats: clock.seats,
                  version: clock.version ?? 0,
                });
              }
              break;
            }
            case "presence": {
              const presence = frame as {
                seats?: WsSeatPresence[];
              };
              if (presence.seats) {
                this.callbacks.onEvent({
                  kind: "presence",
                  seats: presence.seats,
                });
              }
              break;
            }
            case "chat": {
              const chat = frame as {
                seat?: number;
                type?: "text" | "emoji";
                content?: string;
              };
              if (
                typeof chat.seat === "number" &&
                (chat.type === "text" || chat.type === "emoji") &&
                typeof chat.content === "string"
              ) {
                this.callbacks.onEvent({
                  kind: "chat",
                  seat: chat.seat,
                  messageType: chat.type,
                  content: chat.content,
                });
              }
              break;
            }
            case "error": {
              const failure = frame as {
                command_id?: string | null;
                code?: string;
                message?: string;
              };
              /* 只报自己发出去的那条指令；连接层面的错由重连自己处理。 */
              if (failure.command_id) {
                this.callbacks.onEvent({
                  kind: "command_rejected",
                  code: failure.code ?? "unknown",
                  message: failure.message ?? "操作没有生效",
                });
              }
              break;
            }
            case "pong":
              if (this.pingSentAt != null) {
                const milliseconds = Math.max(
                  0,
                  Math.round(performance.now() - this.pingSentAt),
                );
                this.pingSentAt = null;
                this.callbacks.onEvent({ kind: "latency", milliseconds });
              }
              break;
            case "command_result":
              break;
          }
        } catch {
          /* Ignore malformed frames; the server sends valid JSON. */
        }
      };

      ws.onclose = () => {
        this.ws = null;
        this.stopLatencyProbe();
        if (this.state !== "disconnected") {
          this.setState("disconnected");
          this.callbacks.onEvent({ kind: "disconnected" });
          this.scheduleReconnect();
        }
      };

      ws.onerror = () => {
        /* onclose fires after onerror; reconnect is handled there. */
      };
    } catch (error) {
      // Don't reconnect if the session is invalid (logged in elsewhere).
      if (error instanceof ApiError && error.code === "auth.invalid_session") {
        this.setState("disconnected");
        this.callbacks.onEvent({ kind: "disconnected" });
        return;
      }
      this.setState("disconnected");
      this.callbacks.onEvent({ kind: "disconnected" });
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    this.clearReconnect();
    const delay = Math.min(500 * this.backoff, 60000);
    this.backoff = Math.min(this.backoff * 2, 120);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.doConnect();
    }, delay);
  }

  private clearReconnect(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private startLatencyProbe(): void {
    this.stopLatencyProbe();
    this.sendLatencyPing();
    this.latencyTimer = setInterval(() => this.sendLatencyPing(), 5000);
  }

  private sendLatencyPing(): void {
    if (
      this.pingSentAt != null ||
      !this.ws ||
      this.ws.readyState !== WebSocket.OPEN
    ) {
      return;
    }
    this.pingSentAt = performance.now();
    this.ws.send(JSON.stringify({ kind: "ping" }));
  }

  private stopLatencyProbe(): void {
    if (this.latencyTimer !== null) {
      clearInterval(this.latencyTimer);
      this.latencyTimer = null;
    }
    this.pingSentAt = null;
  }

  private setState(state: WsConnState): void {
    if (this.state !== state) {
      this.state = state;
      this.callbacks.onStateChange(state);
    }
  }
}
