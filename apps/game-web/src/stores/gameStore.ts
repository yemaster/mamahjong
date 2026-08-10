import { create } from "zustand";
import type { MatchView, SeatClockView, WsSeatPresence } from "../types";

export type WsState = "disconnected" | "connecting" | "connected";

export interface GameState {
  /** The full observer match view (last fetched from HTTP). */
  matchView: MatchView | null;
  /** The match version from the last view / command result. */
  version: number;
  /** Per-seat clock countdowns (from clock.v1 frames). */
  clocks: Map<number, SeatClockView>;
  /** Local receipt time used to interpolate the visible countdown. */
  clockUpdatedAt: number;
  /** Per-seat online status (from presence.v1 frames). */
  presence: Map<number, boolean>;
  /** WebSocket connection state. */
  wsState: WsState;

  setMatchView: (view: MatchView) => void;
  updateClocks: (seats: SeatClockView[]) => void;
  updatePresence: (seats: WsSeatPresence[]) => void;
  setWsState: (state: WsState) => void;
  reset: () => void;
}

export const useGameStore = create<GameState>((set) => ({
  matchView: null,
  version: 0,
  clocks: new Map(),
  clockUpdatedAt: Date.now(),
  presence: new Map(),
  wsState: "disconnected",

  setMatchView: (view: MatchView) => {
    set({
      matchView: view,
      version: view.version,
      clocks: new Map(
        (view.clocks ?? []).map((clock) => [clock.seat, clock]),
      ),
      clockUpdatedAt: Date.now(),
    });
  },

  updateClocks: (seats: SeatClockView[]) => {
    set((prev) => {
      const clocks = new Map(prev.clocks);
      for (const seat of seats) {
        clocks.set(seat.seat, seat);
      }
      return { clocks, clockUpdatedAt: Date.now() };
    });
  },

  updatePresence: (seats: WsSeatPresence[]) => {
    set((prev) => {
      const presence = new Map(prev.presence);
      for (const seat of seats) {
        presence.set(seat.seat, seat.online);
      }
      return { presence };
    });
  },

  setWsState: (wsState: WsState) => {
    set({ wsState });
  },

  reset: () => {
    set({
      matchView: null,
      version: 0,
      clocks: new Map(),
      clockUpdatedAt: Date.now(),
      presence: new Map(),
      wsState: "disconnected",
    });
  },
}));
