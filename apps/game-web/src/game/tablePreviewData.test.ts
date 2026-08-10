import { describe, expect, it } from "vitest";
import { meldDisplayTiles, tableRelativeSeat } from "./table";
import { tablePreviewView } from "./tablePreviewData";

describe("牌桌调试预览数据", () => {
  it("固定展示包含牌河、副露和宝牌的四人中盘", () => {
    expect(tablePreviewView.players).toHaveLength(4);
    expect(tablePreviewView.remaining_live_draws).toBeLessThan(50);
    expect(tablePreviewView.dora_indicators).toHaveLength(3);
    expect(
      tablePreviewView.players.every(
        (player) => player.discards.length >= 8,
      ),
    ).toBe(true);
    expect(
      tablePreviewView.players.some((player) => player.melds.length > 0),
    ).toBe(true);
    const upperPlayer = tablePreviewView.players.find(
      (player) => player.seat === 3,
    );
    expect(upperPlayer?.melds.map((meld) => meld.kind)).toEqual([
      "added_kan",
      "open_kan",
    ]);
  });

  it("主视角拥有可操作的完整牌面数据", () => {
    const self = tablePreviewView.players.find(
      (player) => player.seat === tablePreviewView.observer_seat,
    );
    expect(self?.concealed_tiles?.length).toBeGreaterThan(0);
    expect(self?.drawn_tile_id).not.toBeNull();
    expect(tablePreviewView.phase).toEqual({
      kind: "awaiting_discard",
      seat: tablePreviewView.observer_seat,
    });
  });

  it("下家用三万五万吃主视角舍出的四万", () => {
    const self = tablePreviewView.players.find((player) => player.seat === 0)!;
    const right = tablePreviewView.players.find((player) => player.seat === 1)!;
    const chi = right.melds.find((meld) => meld.kind === "chi")!;
    const calledTile = chi.tiles.find((tile) => tile.id === chi.called_tile_id);

    expect(chi.called_from).toBe(0);
    expect(calledTile?.code).toBe("4m");
    expect(
      chi.tiles
        .filter((tile) => tile.id !== chi.called_tile_id)
        .map((tile) => tile.code),
    ).toEqual(["3m", "5m"]);
    expect(
      self.discards.some(
        (discard) => discard.tile.code === "4m" && discard.claimed_by === 1,
      ),
    ).toBe(true);
    expect(
      meldDisplayTiles(
        chi,
        tableRelativeSeat(
          chi.called_from!,
          right.seat,
          tablePreviewView.players.length,
        ),
      ).map(({ tile }) => tile.code),
    ).toEqual(["5m", "3m", "4m"]);
  });
});
