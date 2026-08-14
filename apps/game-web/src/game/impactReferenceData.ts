/*
 * 冲击麻将的帮助内容。
 *
 * 和立直那份 `yakuReferenceData.ts` 分开写：两套规则的番种毫无交集，牌型示例的
 * 画法也对不上（冲击麻将的和牌形里可能夹着财神、可能是 15 张 16 张），硬凑成一
 * 张表只会让人以为它们有关系。这边就走纯文字，条目名和结算面板、建房开关一字
 * 不差，玩家在牌桌上看到什么，翻到这里就能查到同一个词。
 */

export interface ImpactRuleItem {
  term: string;
  detail: string;
}

export interface ImpactRuleSection {
  title: string;
  items: ImpactRuleItem[];
}

export interface ImpactYakuItem {
  name: string;
  value: string;
  description: string;
}

export const impactReferenceTabs = [
  "基本规则",
  "记分",
  "全交",
] as const;

export type ImpactReferenceTab = (typeof impactReferenceTabs)[number];

export const impactRuleSections: Record<
  "基本规则",
  ImpactRuleSection[]
> = {
  基本规则: [
    {
      title: "基本流程",
      items: [
        {
          term: "牌局周期",
          detail:
            "初始每人拥有 100 点点数，点数不会为负数。当有人无法支付足够点数时，点数归 0，牌局结束。"
        },
        {
          term: "点数支付",
          detail:
            "分为放铳点炮和自摸两种情况。如果为自摸，则其他三家需要向胡牌家支付胡牌对应的点数。如果为放铳，则放铳者需要向胡牌家支付胡牌对应的点数，非放铳者需要向胡牌家支付胡牌对应点数的一半(向上取整)。\n瞎子模式无法荣和他家。"
        },
        {
          term: "连庄(上塘)",
          detail:
            "庄家胡牌后，可以进行连庄。每次连庄，胡牌基础点数都加 10(对所有人有效)。"
        },
        {
          term: "副露",
          detail:
            "碰优先级大于吃。瞎子模式无法吃；亮子模式可以吃，且自己从另外每一家的舍牌形成副露最多 2 次（吃、碰、明杠合并计数）。"
        },
        {
          term: "荒牌流局",
          detail:
            "如果摸到最后一张牌仍无人胡牌，则本局流局，直接开始下一局。"
        },
        {
          term: "同巡振听",
          detail:
            "放弃荣和某一家时，在自己下一次摸牌之前，不能荣和其他家。"
        }
      ]
    },
    {
      title: "和牌牌型",
      items: [
        {
          term: "平胡",
          detail:
            "最普通的胡牌牌型，即手牌可以组成 1 组雀头和 4 组面子的形式。雀头即相同的 2 张牌，面子分为顺子和刻子。顺子为相邻的三张数牌，刻子为完全相同的三张牌。",
        },
        {
          term: "七对子",
          detail:
            "即手牌可以构成 7 组对子，对子可以重复。对子为相同的两张牌。",
        },
        {
          term: "一杠一达和二杠二达",
          detail:
            "一个杠可以算两个对子。一杠 + 五对 + 任意一张（15 张）、两杠 + 三对 + 任意两张（16 张）都算七对子胡牌牌型；副露必须全部是杠，只要还有碰牌或吃牌就不成立。",
        },
        {
          term: "十三不搭",
          detail:
            "没有对子，且任意两张同花色的数牌相差大于 2。此牌型为门前清限定。",
        },
        {
          term: "七嵌(需要特殊开启",
          detail:
            "手牌能拆成 7 组，每组是同花色、相差恰好为 2 的两张数牌。需要建房时打开「七嵌」。",
        },
      ],
    },
    {
      title: "财神(龙)",
      items: [
        {
          term: "指示牌规则",
          detail:
            "牌山中翻开的是财神指示牌，它的下一张为财神牌。数牌 1→2→…→9→1；风牌 东→南→西→北→东；三元牌 中→发→白→中。",
        },
        {
          term: "百搭",
          detail:
            "财神可以当任意一张牌用，十三不搭与七嵌里同样适用。",
        },
      ],
    },
    {
      title: "杠",
      items: [
        {
          term: "杠点",
          detail:
            "杠点为额外计算项目，初始各家为 0。明杠时，被杠者需要向杠者支付 3 个杠点。暗杠时，其余三家向杠者各支付 2 个杠点。",
        },
        {
          term: "三副露时，碰牌也需支付杠点(需额外开启)",
          detail: "当某一家三副露时，碰牌时，被碰者也需支付 3 个杠点，如果为明杠则需支付 6 个杠点。",
        },
        {
          term: "指示牌碰与暗杠(需额外开启)",
          detail:
            "由于翻出指示牌在牌山中仅有 3 张，因此指示牌碰出时，被碰者也需要支付杠点。当手中拥有 3 张指示牌时，也可以进行“暗杠”。",
        },
        {
          term: "第一巡连打(需额外开启)",
          detail:
            "第一巡四家打了相同的一张牌（如果为指示牌，则只需要庄家和其中两家打了指示牌，需开启指示牌碰牌算杠），则庄家需要向其余 3 家支付 1 个杠点。",
        },
      ],
    },
  ],
};

/** 底和是每一次和牌的起点，番种在它上面加。 */
export const IMPACT_BASE_VALUE_TEXT = "底和 12 点，下列牌型在它之上累加。";

export const impactYakuEntries: ImpactYakuItem[] = [
  { name: "无财神", value: "+1", description: "整副手牌里一张财神都没有。" },
  { name: "两财神", value: "+1", description: "手牌里恰好两张财神。" },
  { name: "三财神", value: "+2", description: "手牌里恰好三张财神。" },
  {
    name: "七对子",
    value: "+1",
    description: "七个对子。一个杠顶两个对子，所以 15 张、16 张也可能成立。",
  },
  {
    name: "七嵌",
    value: "+1",
    description: "七组同花色、相差恰好为 2 的数牌。需要开启「七嵌」。",
  },
  { name: "对对和", value: "+1", description: "四组全是刻子或杠，加一对将。" },
  {
    name: "十三不搭",
    value: "+1",
    description: "无对子，且任意两张同花色数牌相差都大于 2。",
  },
  {
    name: "七风齐",
    value: "+1",
    description: "十三不搭里东南西北中发白恰好各一张，财神不计入。",
  },
  {
    name: "清一色",
    value: "+10",
    description:
      "全手同一花色。若有财神且财神当了别的牌算普通清一色，否则为「无龙清一色」。",
  },
  {
    name: "抛龙",
    value: "+10",
    description:
      "四组面子（或六对）已经做完，多出一张财神单钓将，摸到任意一张牌都能和。一杠一达或两杠两达开杠时，不算抛龙。",
  },
  {
    name: "杠上开花",
    value: "+10",
    description:
      "杠完摸岭上牌直接自摸。",
  },
  {
    name: "抢杠",
    value: "+10",
    description:
      "加杠时，加杠牌被别人荣和。被抢杠者需要支付 3 倍点数，其余玩家无需支付点数，并向其余玩家支付 1 杠点。",
  },
  {
    name: "单吊",
    value: "+10",
    description:
      "四组副露或杠已经摆出，手上只剩一张牌钓将。若「单吊全交」开着则改为全交。",
  },
  {
    name: "连庄",
    value: "+10 / 次",
    description: "当前连庄次数每一次加 10 点，无论那是谁的连庄。",
  },
];

export const IMPACT_ALL_IN_TEXT =
  "触发全交时和牌者 400 点、其余三家归零。牌型类的全交若在建房时关掉，改为该牌型记 +10 点；触发器类（连打十一风、三杠）关掉后就不存在这条路。";

export const impactAllInEntries: ImpactYakuItem[] = [
  {
    name: "连打十一风全交",
    value: "触发",
    description:
      "同一家连续打出 11 张字牌或财神。中间被别人鸣牌不打断，但必须是自己连着打出来的。",
  },
  { name: "全风全交", value: "牌型", description: "整副手牌只有字牌，财神可以当字牌用。" },
  {
    name: "无龙清一色全交",
    value: "牌型",
    description: "清一色，且没有财神、或财神没有当别的牌用。",
  },
  {
    name: "单吊全交",
    value: "牌型",
    description: "四组副露或杠已成，手上只剩一张牌钓将。",
  },
  {
    name: "三杠全交",
    value: "触发",
    description: "同一局内做出三个杠立即触发。指示牌碰与指示牌暗杠不计入。",
  },
  {
    name: "四龙全交",
    value: "触发",
    description: "四张财神在手立即和牌。财神不能被杠，所以只可能是手里的四张。",
  },
  {
    name: "清七对全交",
    value: "牌型",
    description:
      "手里恰好七个对子，且没有财神、或财神没有当别的牌用。一杠一达、二杠二达时不算清七对。",
  },
  { name: "海底全交", value: "牌型", description: "最后一轮摸牌时自摸。" },
  {
    name: "天和地和全交",
    value: "牌型",
    description: "庄家起手即和为天和；闲家第一次摸牌即和为地和。",
  },
];
