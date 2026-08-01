import { Assets, Container, Graphics, Sprite, Text, TextStyle } from "pixi.js";

const TILE_W = 48;
const TILE_H = 64;
const RADIUS = 4;

/** Per-suit background colour for placeholder tiles. */
function suitColor(code: string): number {
  const suit = code.charAt(code.length - 1);
  switch (suit) {
    case "m":
      return 0x8b2500; // man — dark red
    case "p":
      return 0x1e3a5f; // pin — dark blue
    case "s":
      return 0x1e4d2b; // sou — dark green
    default:
      return 0x2a2a3e; // honour — dark grey
  }
}

/** Very abbreviated Unicode label for placeholder tiles. */
function tileLabel(code: string): string {
  const suit = code.charAt(code.length - 1);
  const num = code.slice(0, -1);
  const suitLabels: Record<string, string> = {
    m: "萬",
    p: "筒",
    s: "索",
    z: "",
  };
  const honour: Record<string, string> = {
    "1": "東",
    "2": "南",
    "3": "西",
    "4": "北",
    "5": "白",
    "6": "發",
    "7": "中",
  };
  if (suit === "z") return honour[num] ?? "?";
  if (num === "0" || (num === "5" && code.endsWith("r")))
    return suitLabels[suit]!;
  return `${num}${suitLabels[suit] ?? suit}`;
}

function isRedFive(code: string): boolean {
  return code.endsWith("r");
}

const textStyle = new TextStyle({
  fontFamily: "sans-serif",
  fontSize: 16,
  fill: 0xf5eed6,
  align: "center",
});

/**
 * Creates PixiJS display objects for mahjong tiles.
 *
 * Tries to load a spritesheet from `/assets/tiles/tiles.json`.
 * Falls back to placeholder graphics (coloured rectangles with
 * Unicode labels) when no textures are available.
 */
export class TileFactory {
  private constructor(private readonly atlas: unknown | null) {}

  static async create(): Promise<TileFactory> {
    try {
      const atlas = await Assets.load("/assets/tiles/tiles.json");
      return new TileFactory(atlas);
    } catch {
      return new TileFactory(null);
    }
  }

  createTile(code: string, size?: number): Container {
    if (this.atlas) {
      try {
        const textures = (this.atlas as { textures: Record<string, unknown> })
          .textures;
        if (textures[code]) {
          const sprite = new Sprite(textures[code] as never);
          if (size) {
            sprite.width = size;
            sprite.height = (size * TILE_H) / TILE_W;
          }
          return new Container().addChild(sprite);
        }
      } catch {
        /* fall through to placeholder */
      }
    }
    return this.placeholderTile(code, size ?? TILE_W);
  }

  createBack(size?: number): Container {
    const w = size ?? TILE_W;
    const h = (w * TILE_H) / TILE_W;
    const bg = new Graphics();
    bg.roundRect(0, 0, w, h, RADIUS);
    bg.fill(0x1a3a2e);
    bg.stroke({ width: 1, color: 0x2a5a4e });
    return new Container().addChild(bg);
  }

  private placeholderTile(code: string, size: number): Container {
    const h = (size * TILE_H) / TILE_W;
    const container = new Container();
    const bg = new Graphics();
    bg.roundRect(0, 0, size, h, RADIUS);

    const color = suitColor(code);
    bg.fill(color);

    if (isRedFive(code)) {
      bg.stroke({ width: 2, color: 0xc44b4b });
    } else {
      bg.stroke({ width: 1, color: 0x444444 });
    }

    const label = new Text({ text: tileLabel(code), style: textStyle });
    label.anchor.set(0.5);
    label.x = size / 2;
    label.y = h / 2;

    container.addChild(bg, label);
    container.width = size;
    container.height = h;
    return container;
  }
}

/** Named tile sizes used across the table. */
export const TILE_SELF = 56;
export const TILE_OPPONENT = 48;
export const TILE_DISCARD = 36;
export const TILE_MELD = 40;
export const TILE_DORA = 44;
