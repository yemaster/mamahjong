import { useEffect, useRef } from "react";
import { Application, Container } from "pixi.js";
import { TableLayout } from "./TableLayout";
import {
  TileFactory,
  TILE_SELF,
  TILE_OPPONENT,
  TILE_DISCARD,
  TILE_MELD,
  TILE_DORA,
} from "./TileSprite";
import type { MatchView } from "../types";

const BG_COLOR = 0x0d2818;

interface GameTableProps {
  view: MatchView;
}

/**
 * PixiJS-based mahjong table.
 *
 * Creates the Application on mount (keyed by match id) and
 * imperatively updates sprites when the view changes.
 */
export function GameTable({ view }: GameTableProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const layoutRef = useRef<TableLayout | null>(null);
  const factoryRef = useRef<TileFactory | null>(null);
  const stageRef = useRef<Container | null>(null);

  /* ── Mount / unmount ──────────────────── */

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let destroyed = false;
    const app = new Application();

    (async () => {
      await app.init({
        resizeTo: container,
        background: BG_COLOR,
        antialias: true,
      });
      if (destroyed) {
        app.destroy(true);
        return;
      }
      container.appendChild(app.canvas);

      const factory = await TileFactory.create();
      const layout = new TableLayout(
        view.players.length,
        { width: app.screen.width, height: app.screen.height },
      );
      const stage = new Container();
      app.stage.addChild(stage);

      appRef.current = app;
      layoutRef.current = layout;
      factoryRef.current = factory;
      stageRef.current = stage;

      renderTable(stage, layout, factory, view);
    })();

    return () => {
      destroyed = true;
      app.destroy(true);
      appRef.current = null;
    };
    /* Only recreate on match-id change. */
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.id]);

  /* ── Update on view change ────────────── */

  useEffect(() => {
    const stage = stageRef.current;
    const layout = layoutRef.current;
    const factory = factoryRef.current;
    if (!stage || !layout || !factory) return;

    /* Resize layout to current canvas size. */
    const app = appRef.current!;
    layout = new TableLayout(view.players.length, {
      width: app.screen.width,
      height: app.screen.height,
    });
    layoutRef.current = layout;

    stage.removeChildren();
    renderTable(stage, layout, factory, view);
  }, [view]);

  return (
    <div
      ref={containerRef}
      style={{ width: "100%", height: "100%" }}
    />
  );
}

/* ── Rendering ──────────────────────────── */

function renderTable(
  stage: Container,
  layout: TableLayout,
  factory: TileFactory,
  view: MatchView,
): void {
  const mySeat = view.observer_seat;

  /* Dora indicators. */
  view.dora_indicators.forEach((dora, i) => {
    const pos = layout.doraPosition(i);
    const tile = factory.createTile(dora.code, TILE_DORA);
    tile.x = pos.x;
    tile.y = pos.y;
    stage.addChild(tile);
  });

  /* Each player. */
  for (const player of view.players) {
    const rel = layout.relativeSeat(player.seat, mySeat);
    const isSelf = player.seat === mySeat;

    /* Concealed hand. */
    if (isSelf && player.concealed_tiles) {
      const tiles = player.concealed_tiles;
      const hasDrawn = player.drawn_tile_id != null;
      const handCount = hasDrawn ? tiles.length - 1 : tiles.length;
      const start = layout.handPosition(player.seat, mySeat, handCount);
      for (let i = 0; i < tiles.length; i++) {
        const isDrawn = hasDrawn && i === tiles.length - 1;
        const idx = isDrawn ? handCount : i;
        const pos = layout.handTilePos(start.x, start.y, idx);
        const tile = factory.createTile(tiles[i]!.code, TILE_SELF);
        tile.x = isDrawn ? pos.x + 12 : pos.x;
        tile.y = isDrawn ? pos.y - 4 : pos.y;
        stage.addChild(tile);
      }
    } else {
      /* Opponents: show tile backs. */
      const count = player.concealed_tile_count;
      const start = layout.handPosition(player.seat, mySeat, count);
      for (let i = 0; i < count; i++) {
        const pos = layout.handTilePos(start.x, start.y, i);
        const back = factory.createBack(TILE_OPPONENT);
        back.x = pos.x;
        back.y = pos.y;
        stage.addChild(back);
      }
    }

    /* Discards. */
    for (let i = 0; i < player.discards.length; i++) {
      const pos = layout.discardPosition(player.seat, mySeat, i);
      const disc = player.discards[i]!;
      const tile = factory.createTile(disc.tile.code, TILE_DISCARD);
      tile.x = pos.x;
      tile.y = pos.y;
      if (disc.tsumogiri) {
        tile.rotation = Math.PI / 6; // slight tilt for tsumogiri
      }
      if (disc.riichi_declared) {
        tile.rotation = Math.PI / 2; // sideways for riichi declaration
      }
      stage.addChild(tile);
    }

    /* Melds. */
    for (let m = 0; m < player.melds.length; m++) {
      const meld = player.melds[m]!;
      const pos = layout.meldPosition(
        player.seat,
        mySeat,
        player.concealed_tile_count,
        m,
        meld.tiles.length,
      );
      for (let t = 0; t < meld.tiles.length; t++) {
        const tileCode = meld.tiles[t]!.code;
        const tile = factory.createTile(tileCode, TILE_MELD);
        tile.x = pos.x + t * (TILE_MELD + 4);
        tile.y = pos.y;
        if (
          meld.called_from !== null &&
          t === meld.called_tile_id
        ) {
          tile.rotation = Math.PI / 2;
        }
        stage.addChild(tile);
      }
    }
  }
}
