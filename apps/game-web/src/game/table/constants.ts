/**
 * 三维牌桌的尺寸常量。
 *
 * 牌桌被拆成了若干模块（牌墙、手牌、牌河、副露……），这些数字必须处处一致，
 * 所以统一放在这里，不再散落到各个文件里各写一份。
 */

export const TABLE_IMAGE_SIDE = 16.2;
export const TABLE_CLOTH_SIDE = 12;
export const TABLE_WIDTH = TABLE_IMAGE_SIDE;
export const TABLE_DEPTH = TABLE_IMAGE_SIDE;

export const DEFAULT_TABLECLOTH_ASSET =
  "/game/assets/local-game-assets/mahjong-soul/tablecloths/peacock-green-table.png";

export const TILE_WIDTH_RATIO = 0.72;
export const TILE_DEPTH_RATIO = 0.5;
export const DEFAULT_TILE_SCALE = 0.96;

export const TABLE_TILE_LENGTH = 0.56;
export const TILE_LENGTH = TABLE_TILE_LENGTH;
export const OPPONENT_TILE_LENGTH = TABLE_TILE_LENGTH;
export const MELD_TILE_LENGTH = TABLE_TILE_LENGTH;
export const WALL_TILE_LENGTH = TABLE_TILE_LENGTH;
export const RIVER_TILE_LENGTH = TABLE_TILE_LENGTH;
export const RIVER_TILE_GAP = 0.03;
export const WALL_TILE_DEPTH = WALL_TILE_LENGTH * TILE_DEPTH_RATIO;
export const RIVER_TILE_DEPTH = RIVER_TILE_LENGTH * TILE_DEPTH_RATIO;

export const MELD_RIGHT_EDGE = 4.6;
export const MELD_DISTANCE = 5.35;
export const MELD_GROUP_GAP = 0.1;

export const WALL_STACK_GAP = 0;
export const WALL_STACKS_PER_SIDE = 17;
export const TOTAL_WALL_TILES = WALL_STACKS_PER_SIDE * 2 * 4;
export const WALL_DISTANCE = 4.1;

export const HAND_DISTANCE = 5.35;
/** 暗手相邻两张牌之间的额外空隙；摸入牌的独立间隔由布局另外控制。 */
export const HAND_TILE_GAP = 0.018;

export const OPENING_DEAL_STEP_MS = 120;
export const OPENING_DEAL_MOVE_MS = 180;
/** 摸到手上之后把牌从平躺翻起来立住的时长。 */
export const TILE_STAND_UP_MS = 190;
