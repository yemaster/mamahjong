/*
 * 四川麻将（血战到底）的帮助内容。
 *
 * 和立直、冲击麻将分开写：三套规则的番种与流程都对不上。这里走纯文字，条目名和
 * 结算面板、HUD 徽章一字不差，玩家在牌桌上看到什么，翻到这里就能查到同一个词。
 */

export interface SichuanRuleItem {
  term: string;
  detail: string;
}

export interface SichuanRuleSection {
  title: string;
  items: SichuanRuleItem[];
}

export interface SichuanYakuItem {
  name: string;
  value: string;
  description: string;
}

export const sichuanReferenceTabs = [
  "基本规则",
  "番型",
  "记分",
] as const;

export type SichuanReferenceTab = (typeof sichuanReferenceTabs)[number];

export const sichuanRuleSections: Record<
  "基本规则",
  SichuanRuleSection[]
> = {
  基本规则: [
    {
      title: "牌与牌山",
      items: [
        {
          term: "只用万筒索",
          detail:
            "共 108 张（27 种 × 4），没有字牌，也没有翻宝牌和财神。",
        },
        {
          term: "牌墙",
          detail: "东西各 14 墩、南北各 13 墩，共 54 墩。",
        },
      ],
    },
    {
      title: "换三张",
      items: [
        {
          term: "流程",
          detail:
            "发牌后、定缺前，每家选 3 张同花色牌，与配对家对换。超时随机选 3 张同花色。",
        },
        {
          term: "方向",
          detail:
            "由开局骰子决定：骰和 2、6、10 为逆时针（下家），4、8、12 为顺时针（上家），其余为对家。",
        },
      ],
    },
    {
      title: "定缺",
      items: [
        {
          term: "选择",
          detail:
            "换三张后，每家从万、筒、索里选一门定缺，头像右下角大字徽章显示。",
        },
        {
          term: "约束",
          detail:
            "手上有定缺门牌时，只能打定缺门牌；不能碰、明杠定缺门牌，但可以暗杠。",
        },
        {
          term: "花猪",
          detail: "胡牌时牌型不得含定缺门，含三门的花猪不能胡。",
        },
      ],
    },
    {
      title: "血战到底",
      items: [
        {
          term: "流程",
          detail:
            "一家胡后不结束，胡者盖牌退出，其余继续，直到三家胡或牌山摸尽。胡者的下家继续摸打。",
        },
        {
          term: "局制",
          detail: "4 局。首局庄家为东，之后庄家为上一局第一个胡者。",
        },
      ],
    },
  ],
};

/** 番型的分数公式：分数 = 2^(番−1)，封顶 6 番。 */
export const SICHUAN_FAN_NOTE = "分数 = 2^(番−1)，封顶 6 番（32 分）。";

/** 基础番型取最高、不叠加。 */
export const SICHUAN_BASE_NOTE = "基础番型取最高，不叠加。";

export const sichuanBaseYakuEntries: SichuanYakuItem[] = [
  { name: "平胡", value: "1番", description: "普通胡牌。" },
  { name: "对对胡", value: "2番", description: "四副刻/杠，加一对将。" },
  { name: "清一色", value: "3番", description: "整副手牌同一花色。" },
  { name: "七对", value: "3番", description: "七个对子。" },
  { name: "清对", value: "4番", description: "清一色 + 对对胡。" },
  { name: "龙七对", value: "5番", description: "七对里含四张相同（1 根）。" },
  { name: "清七对", value: "5番", description: "清一色 + 七对。" },
  { name: "天地胡", value: "6番", description: "庄家或闲家第一巡自摸。" },
];

/** 加番叠加在基础番型之上。 */
export const SICHUAN_ADD_NOTE = "加番叠加在基础番型之上。";

export const sichuanAddYakuEntries: SichuanYakuItem[] = [
  { name: "自摸", value: "+1", description: "自摸和牌。" },
  { name: "根", value: "+1/根", description: "每个杠加 1 番。" },
  { name: "杠上花", value: "+1", description: "杠完摸岭上牌直接自摸。" },
  { name: "杠上炮", value: "+1", description: "杠完打出的岭上牌被荣和。" },
  { name: "抢杠胡", value: "+1", description: "加杠时，加杠牌被别人荣和。" },
  { name: "金钩钓", value: "+1", description: "四副副露已摆出，手上单钓将。" },
  { name: "海底", value: "+1", description: "摸到最后一张牌胡牌。" },
];

/** 杠与流局结算是番型之外的点数变动。 */
export const SICHUAN_SCORE_NOTE = "杠与流局结算是番型之外的点数变动。";

export const sichuanScoreEntries: SichuanYakuItem[] = [
  { name: "暗杠", value: "2分/家", description: "其余三家各付 2 分。" },
  { name: "明杠（直杠）", value: "2分", description: "放杠者付 2 分。" },
  { name: "加杠（巴杠）", value: "1分/家", description: "其余三家各付 1 分。" },
  { name: "查花猪", value: "8分/家", description: "流局时手牌含三门者，赔其余未胡家各 8 分。" },
  { name: "查大叫", value: "1分/家", description: "流局时未听牌者，赔每位听牌者 1 分。" },
];
