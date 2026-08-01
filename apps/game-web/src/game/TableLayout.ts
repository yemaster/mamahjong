/**
 * Coordinate system for the game table, parameterised by seat count
 * (3 for sanma, 4 for yonma) and canvas dimensions.
 *
 * All functions return pixel positions relative to the PixiJS stage,
 * with the origin at the top-left corner.
 */

const TILE_W = 48;
const TILE_H = 64;
const TILE_GAP = 4;
const TILE_BACK_COLOR = 0x1a3a2e;

export interface TableDimensions {
  width: number;
  height: number;
}

export class TableLayout {
  readonly seatCount: number;
  readonly centerX: number;
  readonly centerY: number;

  /* Derived constants based on seat count. */
  private readonly selfY: number;
  private readonly topY: number;
  private readonly leftX: number;
  private readonly rightX: number;

  constructor(
    seatCount: number,
    readonly dims: TableDimensions,
  ) {
    this.seatCount = seatCount;
    this.centerX = dims.width / 2;
    this.centerY = dims.height / 2;

    this.selfY = dims.height - TILE_H - 24;
    this.topY = 16;
    this.leftX = 16;
    this.rightX = dims.width - 16;
  }

  /**
   * Maps an absolute seat index to a relative position where seat 0
   * (the observer) is at the bottom.
   */
  relativeSeat(absolute: number, mySeat: number): number {
    return (absolute + this.seatCount - mySeat) % this.seatCount;
  }

  /** Centre of a player's seat area. */
  seatPosition(seat: number, mySeat: number): { x: number; y: number } {
    const rel = this.relativeSeat(seat, mySeat);
    if (rel === 0) return { x: this.centerX, y: this.selfY };
    if (rel === 2) return { x: this.centerX, y: this.topY };
    if (rel === 1) return { x: this.rightX, y: this.centerY };
    return { x: this.leftX, y: this.centerY };
  }

  /** Start position of the hand (tiles face-down for opponents). */
  handPosition(
    seat: number,
    mySeat: number,
    tileCount: number,
  ): { x: number; y: number } {
    const pos = this.seatPosition(seat, mySeat);
    const handWidth = tileCount * (TILE_W + TILE_GAP) - TILE_GAP;
    if (seat === mySeat) {
      return { x: pos.x - handWidth / 2, y: pos.y };
    }
    /* Opponents: hand centred above/below their seat position. */
    const rel = this.relativeSeat(seat, mySeat);
    if (rel === 2) {
      return { x: pos.x - handWidth / 2, y: pos.y };
    }
    /* Side seats: stacked vertically (compact). */
    return { x: pos.x - handWidth / 2, y: pos.y - TILE_H / 2 };
  }

  /** Per-tile position for a hand starting at `handStart`. */
  handTilePos(
    handStartX: number,
    handStartY: number,
    index: number,
  ): { x: number; y: number } {
    return {
      x: handStartX + index * (TILE_W + TILE_GAP),
      y: handStartY,
    };
  }

  /** Drawn tile — slightly offset to the right of the hand. */
  drawnTilePos(
    handStartX: number,
    handStartY: number,
    handCount: number,
  ): { x: number; y: number } {
    const lastX = handStartX + handCount * (TILE_W + TILE_GAP);
    return { x: lastX + 4, y: handStartY };
  }

  /** Row/column grid position for a discard in a player's river. */
  discardPosition(
    seat: number,
    mySeat: number,
    index: number,
  ): { x: number; y: number } {
    const cols = 6;
    const col = index % cols;
    const row = Math.floor(index / cols);
    const base = this.seatPosition(seat, mySeat);
    const discW = TILE_W * 0.7;
    const discH = TILE_H * 0.7;
    // Position the river offset from the seat centre.
    return {
      x: base.x - (cols * discW) / 2 + col * (discW + 2) + discW / 2,
      y: base.y + TILE_H / 2 + 12 + row * (discH + 2),
    };
  }

  /** Melds — placed to the right of the hand for self, above for opponents. */
  meldPosition(
    seat: number,
    mySeat: number,
    handCount: number,
    meldIndex: number,
    meldTileCount: number,
  ): { x: number; y: number } {
    const hand = this.handPosition(seat, mySeat, handCount);
    const handWidth = handCount * (TILE_W + TILE_GAP) - TILE_GAP;
    const meldW = meldTileCount * (TILE_W + TILE_GAP) - TILE_GAP;
    if (seat === mySeat) {
      return {
        x: hand.x - meldW - TILE_GAP * 2 - meldIndex * (meldW + 16),
        y: hand.y,
      };
    }
    return {
      x: hand.x + handWidth / 2 - meldW / 2,
      y: hand.y - TILE_H - 8 - meldIndex * (TILE_H + 4),
    };
  }

  /** Dora indicators — centred in the table. */
  doraPosition(index: number): { x: number; y: number } {
    return {
      x: this.centerX - 2 * (TILE_W + TILE_GAP) + index * (TILE_W + TILE_GAP),
      y: this.centerY - TILE_H / 2 - 40,
    };
  }

  /** Tile dimensions. */
  tileSize(): { w: number; h: number } {
    return { w: TILE_W, h: TILE_H };
  }
}

export { TILE_W, TILE_H, TILE_GAP, TILE_BACK_COLOR };
