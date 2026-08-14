import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { MatchView } from "../types";
import { PlayerHand2D } from "./PlayerHand2D";
import { tablePreviewView } from "./tablePreviewData";

describe("PlayerHand2D 振听显示", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("实时视图标记振听时显示，打牌清除标记后立即移除", () => {
    render(handView(true));
    expect(container.querySelector(".match-hand-2d__furiten")?.textContent).toBe(
      "振听",
    );

    render(handView(false));
    expect(container.querySelector(".match-hand-2d__furiten")).toBeNull();
  });

  function render(view: MatchView) {
    act(() =>
      root.render(
        <PlayerHand2D
          view={view}
          openingPhase="play"
          onTileDiscard={() => {}}
          riichiSelecting={false}
        />,
      ),
    );
  }
});

function handView(furiten: boolean): MatchView {
  return {
    ...tablePreviewView,
    players: tablePreviewView.players.map((player) =>
      player.seat === tablePreviewView.observer_seat
        ? { ...player, furiten }
        : player,
    ),
  };
}
