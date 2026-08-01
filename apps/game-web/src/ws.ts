import { gameApi } from "./api";
import type { WsSeatCountdown, WsSeatPresence } from "./types";

const PROTOCOL = "mamahjong.v1";

export interface SeatCountdown {
  seat: number;
  remainingMs: number;
  baseMs: number;
  reserveMs: number;
}

export interface SeatPresence {
  seat: number;
  online: boolean;
}

export type StreamEvent =
  | { kind: "events_arrived" }
  | { kind: "clock"; seats: SeatCountdown[] }
  | { kind: "presence"; seats: SeatPresence[] }
  | { kind: "disconnected" }
  | { kind: "reconnected"; afterSeq: number };

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
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setState("disconnected");
  }

  sendCommand(name: string, payload?: unknown, expectedVersion?: number): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return;
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
  }

  /* ── internals ──────────────────────────── */

  private async doConnect(): Promise<void> {
    try {
      const ticketResp = await gameApi.wsTicket(this.token);
      const host = this.baseUrl
        .replace(/^https?:\/\//, "")
        .replace(/\/$/, "");
      const wsUrl = `ws://${host}/api/v1/ws?ticket=${ticketResp.ticket}`;

      const ws = new WebSocket(wsUrl);
      this.ws = ws;

      ws.onopen = () => {
        ws.send(
          JSON.stringify({
            kind: "hello",
            protocol: PROTOCOL,
            subscriptions: [
              { stream: this.stream, after_seq: this.afterSeq },
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
            case "clock": {
              const clock = frame as {
                seats?: WsSeatCountdown[];
              };
              if (clock.seats) {
                this.callbacks.onEvent({
                  kind: "clock",
                  seats: clock.seats.map((s) => ({
                    seat: s.seat,
                    remainingMs: s.remaining_ms,
                    baseMs: s.base_ms,
                    reserveMs: s.reserve_ms,
                  })),
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
                  seats: presence.seats.map((s) => ({
                    seat: s.seat,
                    online: s.online,
                  })),
                });
              }
              break;
            }
            case "pong":
            case "command_result":
            case "error":
              break;
          }
        } catch {
          /* Ignore malformed frames; the server sends valid JSON. */
        }
      };

      ws.onclose = () => {
        this.ws = null;
        if (this.state !== "disconnected") {
          this.setState("disconnected");
          this.callbacks.onEvent({ kind: "disconnected" });
          this.scheduleReconnect();
        }
      };

      ws.onerror = () => {
        /* onclose fires after onerror; reconnect is handled there. */
      };
    } catch {
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

  private setState(state: WsConnState): void {
    if (this.state !== state) {
      this.state = state;
      this.callbacks.onStateChange(state);
    }
  }
}
