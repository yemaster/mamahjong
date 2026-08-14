import type {
  HandRecord,
  MatchRecord,
  MeldPayload,
  RecordEvent,
  TileDrawnPayload,
} from "./recordTypes";

/**
 * 把牌谱的事件日志摊平成一串能一步步往前走的步骤。
 *
 * 一步就是一次牌桌上看得见的变化。发牌那一段（开局事件加四家的起手）合成一步，
 * 否则光开局就得点四下；立直宣言、翻新宝牌这些各自算一步，因为桌面确实变了。
 * 记账用的事件（一发失效、振听变化、鸣牌应答）不产生步骤，折叠状态时也用不上。
 */

/** 会让牌桌变样的事件，只有这些产生步骤。 */
const STEP_EVENTS = new Set([
  "riichi.tile_drawn",
  "riichi.tile_discarded",
  "riichi.north_extracted",
  "riichi.meld_declared",
  "riichi.kan_completed",
  "riichi.dora_indicator_revealed",
  "riichi.riichi_established",
  "riichi.tsumo_declared",
  "riichi.ron_declared",
  "riichi.abortive_draw_declared",
  "riichi.exhaustive_draw_declared",
]);

export interface ReplayStep {
  /** `MatchRecord.hands` 的下标。 */
  handIndex: number;
  /** 第几巡，从 1 开始；庄家每摸一次牌 +1。 */
  turnIndex: number;
  /** 折叠到 `hand.events` 的这个下标为止（含）。 */
  eventIndex: number;
  /** 这一步是哪一家做的；没有明确主语（翻宝牌、流局）就是 `null`。 */
  seat: number | null;
  label: string;
}

/** 一局里的步骤范围，跳局要用。 */
export interface HandSteps {
  handIndex: number;
  /** 这一局第一步在整串步骤里的下标。 */
  start: number;
  /** 步数。 */
  count: number;
  /** 每一巡的第一步下标，跳巡要用。 */
  turnStarts: number[];
}

function seatOf(event: RecordEvent): number | null {
  const seat = event.payload["seat"];
  return typeof seat === "number" ? seat : null;
}

function meldLabel(event: RecordEvent): string {
  const meld = (event.payload as unknown as MeldPayload).meld;
  switch (meld?.kind) {
    case "chi":
      return "吃";
    case "pon":
      return "碰";
    case "open_kan":
      return "明杠";
    case "concealed_kan":
      return "暗杠";
    case "added_kan":
      return "加杠";
    default:
      return "鸣牌";
  }
}

function stepLabel(event: RecordEvent): string {
  switch (event.name) {
    case "riichi.tile_drawn":
      return "摸牌";
    case "riichi.tile_discarded":
      return "打牌";
    case "riichi.north_extracted":
      return "拔北";
    case "riichi.meld_declared":
    case "riichi.kan_completed":
      return meldLabel(event);
    case "riichi.dora_indicator_revealed":
      return "新宝牌";
    case "riichi.riichi_established":
      return "立直";
    case "riichi.tsumo_declared":
      return "自摸";
    case "riichi.ron_declared":
      return "荣和";
    case "riichi.abortive_draw_declared":
    case "riichi.exhaustive_draw_declared":
      return "流局";
    default:
      return "进行";
  }
}

/** 庄家从活牌区摸牌就算翻过一巡。 */
function startsNewTurn(event: RecordEvent, dealer: number): boolean {
  if (event.name !== "riichi.tile_drawn") return false;
  const payload = event.payload as unknown as TileDrawnPayload;
  return payload.seat === dealer && payload.source === "live_wall";
}

/** 一局的步骤。第一步固定是「开局」，折叠到最后一手起手牌为止。 */
export function handSteps(hand: HandRecord, handIndex: number): ReplayStep[] {
  const events = hand.events ?? [];
  /* 开局那一步要把发牌全部吃进去，所以先找出最后一手起手牌在哪儿。 */
  let dealtThrough = -1;
  for (let index = 0; index < events.length; index += 1) {
    const name = events[index]?.name;
    if (name === "riichi.hand_started" || name === "riichi.initial_hand_dealt") {
      dealtThrough = index;
    } else if (dealtThrough >= 0) {
      break;
    }
  }

  const steps: ReplayStep[] = [
    {
      handIndex,
      turnIndex: 1,
      eventIndex: dealtThrough,
      seat: null,
      label: "开局",
    },
  ];
  let turnIndex = 0;
  for (let index = dealtThrough + 1; index < events.length; index += 1) {
    const event = events[index];
    if (!event || !STEP_EVENTS.has(event.name)) continue;
    if (startsNewTurn(event, hand.dealer)) turnIndex += 1;
    steps.push({
      handIndex,
      turnIndex: Math.max(1, turnIndex),
      eventIndex: index,
      seat: seatOf(event),
      label: stepLabel(event),
    });
  }
  return steps;
}

/** 整份牌谱摊平成一串步骤，按局的先后接起来。 */
export function buildSteps(record: MatchRecord): ReplayStep[] {
  return (record.hands ?? []).flatMap((hand, index) => handSteps(hand, index));
}

/** 每一局在整串步骤里的位置，跳局跳巡都查这张表。 */
export function indexSteps(steps: ReplayStep[]): HandSteps[] {
  const index: HandSteps[] = [];
  steps.forEach((step, position) => {
    let entry = index.at(-1);
    if (!entry || entry.handIndex !== step.handIndex) {
      entry = {
        handIndex: step.handIndex,
        start: position,
        count: 0,
        turnStarts: [],
      };
      index.push(entry);
    }
    entry.count += 1;
    /* 每巡只记第一步；巡数是连着涨的，长度对得上就是新的一巡。 */
    if (entry.turnStarts.length < step.turnIndex) entry.turnStarts.push(position);
  });
  return index;
}

/** 一局的标题，「东1局 2本场」这种。 */
export function handTitle(hand: HandRecord): string {
  const wind =
    hand.round_wind === "east"
      ? "东"
      : hand.round_wind === "south"
        ? "南"
        : hand.round_wind === "west"
          ? "西"
          : "北";
  const honba = hand.honba > 0 ? ` ${hand.honba}本场` : "";
  return `${wind}${hand.round_number}局${honba}`;
}
