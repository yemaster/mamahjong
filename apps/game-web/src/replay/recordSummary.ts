import { mahjongFamilyLabel } from "../ruleTitle";
import type { MatchRecord, MatchRecordSummary } from "./recordTypes";

/** 牌谱标题需要的那几样，列表和重演页都能凑得出来。 */
export interface RecordTitleParts {
  friend_match: boolean | null | undefined;
  /** 麻将种类：规则集 ID 斜杠前那一截。旧牌谱没有，那一段就省掉。 */
  rule_family?: string | null;
  variant: "yonma" | "sanma" | null | undefined;
  match_length: "east_only" | "hanchan" | null | undefined;
  rule_name: string | null | undefined;
}

/**
 * 「好友对战 · 立直麻将 · 四人南 · ML规则」这类标题。
 *
 * 四段：对局来路、麻将种类、人数与长度、规则名。旧牌谱认不出好友还是匹配、也没记
 * 麻将种类，那两段就整个省掉，不猜。段位匹配用的是标准规则，规则名那一段跟着省——
 * 「段位匹配」四个字本身已经把规则说清楚了，再挂一句「标准规则」是重复。
 */
export function matchRecordTitle(record: RecordTitleParts): string {
  const ranked = record.friend_match === false;
  const mode =
    record.friend_match === true ? "好友对战" : ranked ? "段位匹配" : null;
  const family = record.rule_family
    ? mahjongFamilyLabel(record.rule_family)
    : null;
  const seats =
    record.variant === "sanma"
      ? "三人"
      : record.variant === "yonma"
        ? "四人"
        : null;
  const length =
    record.match_length === "east_only"
      ? "东"
      : record.match_length === "hanchan"
        ? "南"
        : null;
  const rule = seats && length ? `${seats}${length}` : (seats ?? length);
  const ruleName = ranked ? null : record.rule_name;
  return [mode, family, rule, ruleName].filter(Boolean).join(" · ") || "对局";
}

/**
 * 从整份牌谱里抠出标题要的那几样。
 *
 * 列表接口是服务端拆好再发的，重演页拿到的是牌谱本体，人数和长度得自己从快照里读；
 * 规则名两边都是服务端现算好挂上来的。
 */
export function recordTitleParts(record: MatchRecord): RecordTitleParts {
  const config = record.rule_snapshot?.config;
  return {
    friend_match: record.friend_match ?? null,
    rule_family: record.rule_snapshot?.rule_set_id?.split("/")[0] ?? null,
    variant: config?.variant ?? null,
    match_length: config?.match_rules?.length ?? null,
    rule_name: record.rule_name ?? null,
  };
}

/** 列表上的时间，本地时区、精确到分钟。 */
export function formatRecordTime(epochMs: number): string {
  if (!epochMs) return "";
  const time = new Date(epochMs);
  const pad = (value: number) => String(value).padStart(2, "0");
  return (
    `${time.getFullYear()}-${pad(time.getMonth() + 1)}-${pad(time.getDate())}` +
    ` ${pad(time.getHours())}:${pad(time.getMinutes())}`
  );
}

/** 名次升序，同名次按座位号，保证列表每次渲染顺序一致。 */
export function orderedSeats(record: MatchRecordSummary) {
  return [...record.seats].sort(
    (left, right) => left.rank - right.rank || left.seat - right.seat,
  );
}
