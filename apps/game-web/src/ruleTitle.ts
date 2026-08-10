import type { RoomView } from "./types";

/**
 * 麻将种类的中文名。建房页、房间页、牌谱标题写的是同一套字。
 *
 * 规则集 ID 是「种类/人数」，斜杠前那一截就是麻将种类。
 */
export const MAHJONG_FAMILY_LABELS: Record<string, string> = {
  riichi: "立直麻将",
  impact: "冲击麻将",
};

/** 冲击麻将的模式名。目前只有瞎子麻将一种，它不带任何额外机制，只是个名字。 */
const IMPACT_MODE_LABELS: Record<string, string> = {
  blind: "瞎子麻将",
};

export function mahjongFamilyOf(ruleSetId: string): string {
  return ruleSetId.split("/")[0] ?? "";
}

/** 认不出的种类原样回显，别猜也别写死一个假名字。 */
export function mahjongFamilyLabel(family: string): string {
  return MAHJONG_FAMILY_LABELS[family] ?? family;
}

/**
 * 房间顶栏那一行：「立直麻将 · 四人南 · A规」「冲击麻将 · 瞎子麻将」。
 *
 * 中间那一段写的是「这桌跟同种麻将的别桌差在哪」——立直是人数与长度，冲击麻将只有
 * 四人一种、也没有半庄之分，能区分的就只剩模式名。
 *
 * 规则名那一段冲击麻将在标准规则下省掉：它没有流派，只有「标准 / 自定义」两种取值，
 * 写上「标准规则」等于没说；改过才写，那才是要提醒人的信息。立直不省，因为「标准规则」
 * 在那边是和 A规、ML规则并列的一个真选项。
 */
export function roomRuleTitle(room: RoomView): string {
  const family = mahjongFamilyLabel(room.variant_kind);
  const config = room.rule_snapshot?.config;

  if (room.variant_kind === "impact") {
    const mode = config?.mode;
    return [
      family,
      mode ? (IMPACT_MODE_LABELS[mode] ?? mode) : null,
      room.rule_name === "标准规则" ? null : room.rule_name,
    ]
      .filter(Boolean)
      .join(" · ");
  }

  const seats = config?.variant === "sanma" ? "三人" : "四人";
  const length = config?.match_rules?.length === "east_only" ? "东" : "南";
  return [family, config ? `${seats}${length}` : null, room.rule_name]
    .filter(Boolean)
    .join(" · ");
}
