import type {
  DiscardView,
  MatchPlayerView,
  MatchView,
  MeldView,
  TileView,
} from "../types";

const avatarBase =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes`;
const characterBase = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul`;
/** 预览用的四家角色，立绘和 id 按座次一一对应。 */
const previewCharacterIds = [
  "ichihime",
  "kujo-riu",
  "fukuhime",
  "yagi-yui",
];
const illustrations = [
  `${characterBase}/ichihime/outfits/yiji.png`,
  `${characterBase}/kujo-riu/outfits/default.png`,
  `${characterBase}/fukuhime/outfits/default.png`,
  `${characterBase}/yagi-yui/outfits/default.png`,
];

function tiles(start: number, codes: string[]): TileView[] {
  return codes.map((code, index) => ({ id: start + index, code }));
}

function discards(
  start: number,
  codes: string[],
  riichiIndex = -1,
  claimedBy: Record<number, number> = {},
): DiscardView[] {
  return tiles(start, codes).map((tile, index) => ({
    tile,
    tsumogiri: index % 3 === 1,
    riichi_declared: index === riichiIndex,
    claimed_by: claimedBy[index] ?? null,
  }));
}

function player(
  seat: number,
  nickname: string,
  points: number,
  concealedCodes: string[] | null,
  melds: MeldView[],
  playerDiscards: DiscardView[],
  avatar: string,
  riichi: MatchPlayerView["riichi_status"] = "none",
  waitingTiles: MatchPlayerView["waiting_tiles"] = [],
): MatchPlayerView {
  const concealed = concealedCodes
    ? tiles(1000 + seat * 100, concealedCodes)
    : null;
  return {
    user_id: `preview-player-${seat}`,
    seat,
    nickname,
    avatar_path: `${avatarBase}/${avatar}.png`,
    character_id: previewCharacterIds[seat] ?? null,
    character_illustration_path: illustrations[seat] ?? null,
    points,
    concealed_tiles: concealed,
    concealed_tile_count:
      concealedCodes?.length ?? Math.max(1, 13 - melds.length * 3),
    drawn_tile_id:
      seat === 0 && concealed && concealed.length > 0
        ? concealed.at(-1)!.id
        : null,
    melds,
    discards: playerDiscards,
    riichi_status: riichi,
    waiting_tiles: waitingTiles,
    furiten: false,
  };
}

const selfPonTiles = tiles(2000, ["7p", "7p", "7p"]);
const rightChiTiles = tiles(2100, ["3m", "5m", "4m"]);
const oppositePonTiles = tiles(2200, ["2s", "2s", "2s"]);
const upperAddedKanTiles = tiles(2300, ["5p", "5p", "5p", "5p"]);
const upperOpenKanTiles = tiles(2400, ["7s", "7s", "7s", "7s"]);

export const tablePreviewView: MatchView = {
  schema: "match.v1",
  variant_kind: "riichi",
  id: "table-preview",
  room_id: "000000",
  version: 18,
  event_sequence: 18,
  hand_index: 1,
  observer_seat: 0,
  progress: {
    round_wind: "east",
    round_number: 2,
    dealer: 1,
    honba: 2,
    riichi_sticks: 1,
  },
  phase: { kind: "awaiting_discard", seat: 0 },
  remaining_live_draws: 34,
  dora_indicators: [
    { id: 3000, code: "4s" },
    { id: 3001, code: "7p" },
    { id: 3002, code: "3z" },
  ],
  players: [
    player(
      0,
      "预览玩家",
      28700,
      ["2m", "3m", "4m", "6m", "7m", "8m", "3p", "4p", "5p", "0s", "5s"],
      [
        {
          id: 4000,
          kind: "pon",
          tiles: selfPonTiles,
          called_from: 3,
          called_tile_id: selfPonTiles[0]!.id,
        },
      ],
      discards(
        5000,
        ["1m", "9m", "1p", "9p", "1s", "9s", "1z", "5z", "4m"],
        -1,
        { 8: 1 },
      ),
      "8",
      "none",
      /* 让预览页能看到右下角的感叹号和长按展开的听牌。 */
      [
        { code: "2p", has_yaku: true },
        { code: "5p", has_yaku: false },
      ],
    ),
    player(
      1,
      "下家",
      23600,
      null,
      [
        {
          id: 4001,
          kind: "chi",
          tiles: rightChiTiles,
          called_from: 0,
          called_tile_id: rightChiTiles[2]!.id,
        },
      ],
      discards(5100, ["9s", "1m", "6z", "3p", "7m", "2z", "8p", "4s", "1p"]),
      "3",
    ),
    player(
      2,
      "对家",
      25800,
      null,
      [
        {
          id: 4002,
          kind: "pon",
          tiles: oppositePonTiles,
          called_from: 1,
          called_tile_id: oppositePonTiles[2]!.id,
        },
      ],
      discards(5200, ["1z", "9p", "3z", "6m", "8s", "2p", "7z", "6s", "5p"], 6),
      "6",
      "established",
    ),
    player(
      3,
      "上家",
      20900,
      null,
      [
        {
          id: 4003,
          kind: "added_kan",
          tiles: upperAddedKanTiles,
          called_from: 2,
          called_tile_id: upperAddedKanTiles[0]!.id,
        },
        {
          id: 4004,
          kind: "open_kan",
          tiles: upperOpenKanTiles,
          called_from: 0,
          called_tile_id: upperOpenKanTiles[2]!.id,
        },
      ],
      discards(5300, ["9m", "1s", "4z", "8m", "6p", "3s", "7p", "5z", "2m"]),
      "10",
    ),
  ],
  available_reactions: [],
  turn_actions: {
    can_tsumo: false,
    riichi_discard_tile_ids: [],
    riichi_discard_hints: [],
    tenpai_discard_hints: [],
    concealed_kan_tile_ids: [],
    added_kan_options: [],
    can_nine_terminals: false,
  },
  clocks: [
    { seat: 0, remaining_ms: 12000, base_ms: 5000, reserve_ms: 15000 },
    { seat: 1, remaining_ms: 18000, base_ms: 5000, reserve_ms: 15000 },
    { seat: 2, remaining_ms: 9000, base_ms: 5000, reserve_ms: 15000 },
    { seat: 3, remaining_ms: 15000, base_ms: 5000, reserve_ms: 15000 },
  ],
  opening_ready_seats: [0, 1, 2, 3],
  hand_settlement: null,
  result: null,
  friend_match: true,
  can_start_exit_vote: true,
  exit_vote: null,
  terminated_by_exit_vote: false,
};
