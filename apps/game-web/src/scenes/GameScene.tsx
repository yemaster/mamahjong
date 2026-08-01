import { useCallback, useEffect, useRef, useState } from "react";
import { gameApi, apiFailure } from "../api";
import { GameTable } from "../game/GameTable";
import { ClockBar } from "../game/ClockBar";
import { ActionPanel } from "../game/ActionPanel";
import { MatchStream } from "../ws";
import { useAuthStore } from "../stores/authStore";
import { useGameStore } from "../stores/gameStore";
import { navigateTo } from "../routing";
import type { GameCommandName } from "../types";

const POLL_MS = 500;

interface GameSceneProps {
  matchId: string;
}

export default function GameScene({ matchId }: GameSceneProps) {
  const token = useAuthStore((s) => s.token);
  const {
    matchView,
    setMatchView,
    updateClocks,
    updatePresence,
    setWsState,
    reset,
    wsState,
  } = useGameStore();
  const streamRef = useRef<MatchStream | null>(null);
  const [selectedTileId, setSelectedTileId] = useState<number | undefined>();
  const [error, setError] = useState<string | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  /* ── Initial HTTP fetch ──────────────── */
  const fetchView = useCallback(async () => {
    if (!token) return;
    try {
      const view = await gameApi.matchView(matchId, token);
      setMatchView(view);
      setError(null);
      if (view.result) {
        setTimeout(() => {
          navigateTo({ kind: "result", matchId });
        }, 1500);
      }
    } catch (err: unknown) {
      setError(apiFailure(err).message);
    }
  }, [token, matchId, setMatchView]);

  /* ── Mount / unmount ─────────────────── */
  useEffect(() => {
    fetchView();
    reset();

    /* Start WebSocket. */
    if (token) {
      const host = window.location.host;
      const baseUrl = `${window.location.protocol}//${host}`;
      const stream = new MatchStream(
        baseUrl,
        token,
        matchId,
        matchView?.event_sequence ?? 0,
        {
          onEvent: (event) => {
            switch (event.kind) {
              case "events_arrived":
                fetchView();
                break;
              case "clock":
                updateClocks(
                  event.seats.map((s) => ({
                    ...s,
                    base_ms: s.baseMs,
                    reserve_ms: s.reserveMs,
                  })),
                );
                break;
              case "presence":
                updatePresence(event.seats);
                break;
              case "disconnected":
                /* Polling fallback starts below. */
                break;
              case "reconnected":
                fetchView();
                break;
            }
          },
          onStateChange: (state) =>
            setWsState(
              state === "connected"
                ? "connected"
                : state === "connecting"
                  ? "connecting"
                  : "disconnected",
            ),
        },
      );
      stream.connect();
      streamRef.current = stream;
    }

    /* HTTP polling fallback while disconnected. */
    pollTimer.current = setInterval(() => {
      const s = useGameStore.getState();
      if (s.wsState === "disconnected") {
        fetchView();
      }
    }, POLL_MS);

    return () => {
      streamRef.current?.disconnect();
      if (pollTimer.current) clearInterval(pollTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [matchId, token]);

  /* ── Commands ─────────────────────────── */
  const onCommand = useCallback(
    (name: GameCommandName, payload?: unknown) => {
      if (!token) return;
      const view = useGameStore.getState().matchView;
      if (!view) return;

      /* Send via WS. */
      streamRef.current?.sendCommand(
        name,
        payload,
        view.version,
      );
      /* Also send via HTTP for authoritative response. */
      gameApi
        .gameCommand(matchId, view.version, name, payload, token)
        .then((v) => setMatchView(v))
        .catch((err: unknown) => setError(apiFailure(err).message));
    },
    [token, matchId, setMatchView],
  );

  /* ── Keyboard ─────────────────────────── */
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!matchView) return;
      const tiles = matchView.players.find(
        (p) => p.seat === matchView.observer_seat,
      )?.concealed_tiles;
      if (!tiles || tiles.length === 0) return;

      if (e.key === "ArrowLeft") {
        setSelectedTileId((prev) => {
          const idx = tiles.findIndex((t) => t.id === prev);
          const next = Math.max(0, idx === -1 ? 0 : idx - 1);
          return tiles[next]!.id;
        });
      } else if (e.key === "ArrowRight") {
        setSelectedTileId((prev) => {
          const idx = tiles.findIndex((t) => t.id === prev);
          const next = Math.min(
            tiles.length - 1,
            idx === -1 ? 0 : idx + 1,
          );
          return tiles[next]!.id;
        });
      } else if (e.key === "Enter") {
        onCommand("riichi.discard", { tile_id: selectedTileId });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [matchView, selectedTileId, onCommand]);

  /* ── Render ───────────────────────────── */

  if (!matchView) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
          color: "var(--color-text-dim)",
        }}
      >
        正在载入对局…
        {error && (
          <div style={{ color: "var(--color-danger)", marginTop: 8 }}>
            {error}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        position: "relative",
        width: "100%",
        height: "100%",
        overflow: "hidden",
      }}
    >
      <GameTable view={matchView} />
      <ClockBar />
      <ActionPanel
        view={matchView}
        onCommand={onCommand}
        selectedTileId={selectedTileId}
      />
      {wsState !== "connected" && (
        <div
          style={{
            position: "absolute",
            top: 4,
            right: 8,
            fontSize: 12,
            color: "var(--color-offline)",
            background: "rgba(0,0,0,0.6)",
            padding: "2px 8px",
            borderRadius: "var(--radius-sm)",
            zIndex: 10,
          }}
        >
          离线
        </div>
      )}
      {error && (
        <div
          style={{
            position: "absolute",
            top: 4,
            left: "50%",
            transform: "translateX(-50%)",
            fontSize: 13,
            color: "var(--color-danger)",
            background: "rgba(0,0,0,0.6)",
            padding: "2px 12px",
            borderRadius: "var(--radius-sm)",
            zIndex: 10,
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}
