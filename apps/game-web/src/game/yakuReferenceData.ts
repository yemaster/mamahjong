export interface YakuReferenceEntry {
  name: string;
  value: string;
  description: string;
  tiles: string[];
  groupStarts?: number[];
  winningTileIndex?: number;
  openReduction?: number;
  menzenRequired?: boolean;
  meldGroups?: {
    start: number;
    length: number;
    calledTileIndex: number;
  }[];
}

export function yakuValueTags(value: string): string[] {
  return value.split("・");
}

export const yakuReferenceTabs = [
  "1番",
  "2番",
  "3番",
  "6番",
  "役满",
  "双倍役满",
] as const;

export type YakuReferenceTab = (typeof yakuReferenceTabs)[number];

export function yakuEntryTabs(
  entry: YakuReferenceEntry,
): YakuReferenceTab[] {
  if (entry.value === "双倍役满") return ["双倍役满"];
  if (entry.value === "役满") return ["役满"];
  const values = Array.from(entry.value.matchAll(/(\d+)番/g), (match) =>
    Number(match[1]),
  );
  const closedValue = Math.max(...values);
  return [`${closedValue}番` as YakuReferenceTab];
}

function tiles(pattern: string): string[] {
  return pattern
    .trim()
    .split(/\s+/)
    .flatMap((group) => {
      const suit = group.slice(-1);
      return group
        .slice(0, -1)
        .split("")
        .map((rank) => `${rank}${suit}`);
    });
}

const ordinary = "123m 456m 234p 678s 55p";

export const yakuReferenceEntries: YakuReferenceEntry[] = [
  {
    name: "门前清自摸和",
    value: "1番",
    menzenRequired: true,
    description: "门前清状态下自摸和牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "立直",
    value: "1番",
    menzenRequired: true,
    description: "门前听牌后宣告立直并完成和牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "一发",
    value: "1番",
    menzenRequired: true,
    description: "立直后的一巡内和牌，期间没有鸣牌或开杠。",
    tiles: tiles(ordinary),
  },
  {
    name: "自风",
    value: "1番",
    description: "持有与自己座风相同的刻子或杠子。",
    tiles: tiles("234m 456m 67p 55s 111z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "场风",
    value: "1番",
    description: "持有与当前场风相同的刻子或杠子。",
    tiles: tiles("234m 456m 67p 55s 222z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "役牌白",
    value: "1番",
    description: "持有白的刻子或杠子。",
    tiles: tiles("234m 456m 67p 55s 555z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "役牌发",
    value: "1番",
    description: "持有发的刻子或杠子。",
    tiles: tiles("234m 456m 67p 55s 666z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "役牌中",
    value: "1番",
    description: "持有中的刻子或杠子。",
    tiles: tiles("234m 456m 67p 55s 777z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "平和",
    value: "1番",
    menzenRequired: true,
    description: "门前的四组顺子、非役牌雀头和两面听牌。",
    tiles: tiles("123m 456m 234p 67s 55p 8s"),
    groupStarts: [3, 6, 9, 11],
    winningTileIndex: 13,
  },
  {
    name: "断幺九",
    value: "1番",
    description: "整副牌只由二至八的数牌组成。",
    tiles: tiles("234m 345m 456p 678s 55p"),
  },
  {
    name: "一杯口",
    value: "1番",
    menzenRequired: true,
    description: "门前牌中含有同色同数字的两组顺子。",
    tiles: tiles("123m 123m 456p 789s 55p"),
  },
  {
    name: "海底摸月",
    value: "1番",
    description: "摸取牌山最后一张牌时自摸和牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "河底捞鱼",
    value: "1番",
    description: "荣和本局最后一张舍牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "抢杠",
    value: "1番",
    description: "荣和其他玩家加杠时使用的牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "岭上开花",
    value: "1番",
    description: "开杠后以岭上牌自摸和牌。",
    tiles: tiles("234p 456p 67s 55p 1111m 8s"),
    meldGroups: [{ start: 10, length: 4, calledTileIndex: 10 }],
    winningTileIndex: 14,
  },
  {
    name: "两立直",
    value: "2番",
    menzenRequired: true,
    description: "第一巡且无人鸣牌时宣告立直。",
    tiles: tiles(ordinary),
  },
  {
    name: "七对子",
    value: "2番",
    menzenRequired: true,
    description: "由七组不同的对子组成。",
    tiles: tiles("11m 22m 33p 44p 55s 66s 77z"),
  },
  {
    name: "对对和",
    value: "2番",
    description: "四组面子全部为刻子或杠子。",
    tiles: tiles("333m 555p 77s 22z 111m 7s"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "三暗刻",
    value: "2番",
    description: "和牌时持有三组未通过碰取得的刻子或杠子。",
    tiles: tiles("111m 333m 555p 678s 22z"),
  },
  {
    name: "三色同刻",
    value: "2番",
    description: "万、筒、索三色中各有一组相同数字的刻子。",
    tiles: tiles("555m 555p 34m 22z 555s 2m"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "三杠子",
    value: "2番",
    description: "和牌时持有三组杠子。",
    tiles: tiles("67s 22z 1111m 3333p 5555s 8s"),
    meldGroups: [
      { start: 4, length: 4, calledTileIndex: 4 },
      { start: 8, length: 4, calledTileIndex: 8 },
      { start: 12, length: 4, calledTileIndex: 12 },
    ],
    winningTileIndex: 16,
  },
  {
    name: "小三元",
    value: "2番",
    description: "两组三元牌刻子，加另一种三元牌雀头。",
    tiles: tiles("666z 77z 234m 67p 555z 8p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "混老头",
    value: "2番",
    description: "整副牌只由幺九牌和字牌组成。",
    tiles: tiles("111m 999m 111p 999s 55z"),
  },
  {
    name: "三色同顺",
    value: "门前2番",
    openReduction: 1,
    description: "万、筒、索三色中各有一组相同数字的顺子。",
    tiles: tiles("234m 234p 67m 55z 234s 8m"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "一气通贯",
    value: "门前2番",
    openReduction: 1,
    description: "同一花色中集齐一二三、四五六和七八九。",
    tiles: tiles("123m 789m 34p 55z 456m 5p"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "混全带幺九",
    value: "门前2番",
    openReduction: 1,
    description: "每组面子和雀头都含幺九牌或字牌，且含有顺子。",
    tiles: tiles("123m 789p 99s 55z 111z 9s"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "二杯口",
    value: "3番",
    menzenRequired: true,
    description: "门前牌中含有两组一杯口。",
    tiles: tiles("123m 123m 678p 678p 55s"),
  },
  {
    name: "混一色",
    value: "门前3番",
    openReduction: 1,
    description: "整副牌只使用一种数牌和字牌。",
    tiles: tiles("123m 789m 11z 55z 456m 1z"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "纯全带幺九",
    value: "门前3番",
    openReduction: 1,
    description: "每组面子和雀头都含幺九牌，且不含字牌。",
    tiles: tiles("123m 789m 99s 11m 111p 9s"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "清一色",
    value: "门前6番",
    openReduction: 1,
    description: "整副牌只由同一种花色的数牌组成。",
    tiles: tiles("123m 789m 22m 55m 456m 2m"),
    meldGroups: [{ start: 10, length: 3, calledTileIndex: 10 }],
    winningTileIndex: 13,
  },
  {
    name: "天和",
    value: "役满",
    menzenRequired: true,
    description: "庄家在第一次摸牌时即自摸和牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "地和",
    value: "役满",
    menzenRequired: true,
    description: "闲家在无人鸣牌前以第一次摸牌自摸和牌。",
    tiles: tiles(ordinary),
  },
  {
    name: "国士无双",
    value: "役满",
    menzenRequired: true,
    description: "集齐十三种幺九字牌，并使其中一种成对。",
    tiles: tiles("19m 19p 19s 1234567z 1m"),
  },
  {
    name: "四暗刻",
    value: "役满",
    menzenRequired: true,
    description: "门前持有四组暗刻或暗杠，并以自摸和牌。",
    tiles: tiles("111m 333m 555p 777s 22z"),
  },
  {
    name: "大三元",
    value: "役满",
    description: "白、发、中三种三元牌均组成刻子或杠子。",
    tiles: tiles("67m 22p 555z 666z 777z 8m"),
    meldGroups: [
      { start: 4, length: 3, calledTileIndex: 4 },
      { start: 7, length: 3, calledTileIndex: 7 },
      { start: 10, length: 3, calledTileIndex: 10 },
    ],
    winningTileIndex: 13,
  },
  {
    name: "绿一色",
    value: "役满",
    description: "整副牌只由二三四六八索和发组成。",
    tiles: tiles("222s 333s 444s 666s 88s"),
  },
  {
    name: "字一色",
    value: "役满",
    description: "整副牌全部由字牌组成。",
    tiles: tiles("111z 222z 555z 666z 77z"),
  },
  {
    name: "小四喜",
    value: "役满",
    description: "三种风牌组成刻子或杠子，第四种风牌作雀头。",
    tiles: tiles("111z 222z 333z 44z 555m"),
  },
  {
    name: "大四喜",
    value: "双倍役满",
    description: "东、南、西、北四种风牌均组成刻子或杠子。",
    tiles: tiles("111z 222z 333z 444z 55m"),
  },
  {
    name: "清老头",
    value: "役满",
    description: "整副牌只由一和九的数牌组成。",
    tiles: tiles("111m 999m 111p 999s 11s"),
  },
  {
    name: "四杠子",
    value: "役满",
    description: "和牌时持有四组杠子。",
    tiles: tiles("2z 1111m 3333p 5555s 7777z 2z"),
    meldGroups: [
      { start: 1, length: 4, calledTileIndex: 1 },
      { start: 5, length: 4, calledTileIndex: 5 },
      { start: 9, length: 4, calledTileIndex: 9 },
      { start: 13, length: 4, calledTileIndex: 13 },
    ],
    winningTileIndex: 17,
  },
  {
    name: "九莲宝灯",
    value: "役满",
    menzenRequired: true,
    description: "门前清一色中具备一一一二三四五六七八九九九结构。",
    tiles: tiles("1112345678999m 5m"),
  },
  {
    name: "国士无双十三面",
    value: "双倍役满",
    menzenRequired: true,
    description: "十三种幺九字牌齐全后，等待任意一种成对。",
    tiles: tiles("19m 19p 19s 1234567z 1m"),
  },
  {
    name: "四暗刻单骑",
    value: "双倍役满",
    menzenRequired: true,
    description: "四组暗刻或暗杠完成后，以单骑等待雀头。",
    tiles: tiles("111m 333m 555p 777s 22z"),
  },
  {
    name: "纯正九莲宝灯",
    value: "双倍役满",
    menzenRequired: true,
    description: "九莲基本形完成后，九面等待同色任意数牌。",
    tiles: tiles("1112345678999m 5m"),
  },
  {
    name: "人和",
    value: "役满",
    menzenRequired: true,
    description: "启用古役时，闲家在第一次摸牌前荣和。",
    tiles: tiles(ordinary),
  },
  {
    name: "大车轮",
    value: "役满",
    menzenRequired: true,
    description: "启用古役时，由二至八筒各一对组成七对子。",
    tiles: tiles("22334455667788p"),
  },
  {
    name: "大竹林",
    value: "役满",
    menzenRequired: true,
    description: "启用古役时，由二至八索各一对组成七对子。",
    tiles: tiles("22334455667788s"),
  },
  {
    name: "大数邻",
    value: "役满",
    menzenRequired: true,
    description: "启用古役时，由二至八万各一对组成七对子。",
    tiles: tiles("22334455667788m"),
  },
];
