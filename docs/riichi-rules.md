# 日麻规则配置

状态：M2 设计基线
最后更新：2026-07-30

## 原则

- 配置按职责分组，不使用无边界的布尔字段列表。
- 配置经过补全和整体校验后才能生成不可变 `RuleSnapshot`。
- 预设只负责生成完整配置，计分及状态机不判断预设名称。
- 四麻、三麻共享配置类型，但人数不可能支持的选项必须拒绝。
- JSON 字段使用稳定 `snake_case`，未知字段拒绝，避免拼写错误被忽略。

## 配置结构

```text
RiichiRules
├── variant
├── match_rules
│   ├── length
│   ├── initial_points
│   ├── return_points
│   ├── tobi
│   ├── dealer_continuation
│   └── agari_yame
├── scoring
│   ├── kiriage_mangan
│   ├── old_yaku
│   ├── yakuman_value
│   ├── nagashi_mangan
│   └── kazoe_yakuman
├── bonuses
│   ├── red_fives
│   ├── ippatsu
│   ├── ura_dora
│   └── kan_dora
├── abortive_draws
│   ├── four_winds
│   ├── four_kans
│   ├── nine_terminals
│   └── four_riichi
└── settlement
    ├── uma
    ├── noten_payment
    └── ron_resolution
```

### match_rules

| 字段 | 类型 | 语义 |
|---|---|---|
| `length` | `east_only / hanchan` | 东风战或东南战 |
| `initial_points` | 正整数 | 每人初始点数，普通默认 25000 |
| `return_points` | 正整数 | 结算返点，用于计算 Oka |
| `tobi` | bool | 小于 0 点时提前结束 |
| `dealer_continuation` | `win_only / win_or_tenpai` | 亲家连庄条件 |
| `agari_yame` | bool | 最后一局亲家第一名和牌后能否结束 |

`initial_points` 和 `return_points` 必须是 1000 的倍数。进行中的玩家点数允许
为负数。

### scoring

| 字段 | 类型 | 语义 |
|---|---|---|
| `kiriage_mangan` | bool | 30符4翻、60符3翻是否切上满贯 |
| `old_yaku` | bool | 是否启用独立古役目录 |
| `yakuman_value` | enum | 特殊形是否算双倍役满 |
| `nagashi_mangan` | bool | 是否有流局满贯 |
| `kazoe_yakuman` | bool | 13翻以上是否累计役满，否则封顶三倍满 |

`yakuman_value`：

- `stacked_only`：不同役满可以叠加，四暗刻单骑、国士十三面、纯正九莲等
  特殊形本身只算一倍；
- `double_variants_and_stacked`：上述特殊形按双倍，且不同役满继续叠加。

### bonuses

赤五分别记录万、筒、索数量，每门 `0..=4`。四麻普通默认各一张；三麻因
二万到八万被移除，万子赤五必须为 0。

`kan_dora` 控制新增杠宝指示牌。`ura_dora` 开启时，立直和牌按实际已翻开的
表宝指示牌数量取得对应里宝；它不要求 `ippatsu` 开启。

### abortive_draws

四风连打和四家立直在三麻不可能成立，三麻配置必须关闭。四杠散了与九种
九牌可以独立配置。具体触发时机由 M3 状态机实现。

### settlement

普通马点按千点为单位：

```json
{"type":"fixed","values":[30,10,-10,-30]}
```

固定马点数量必须等于玩家数，总和必须为 0。Oka 不写入马点数组，而由
`(return_points - initial_points) × 玩家数` 在整场结算时归第一名。

日本职业麻将联盟 A 规则的浮动顺位点使用独立类型，根据正分人数选择官方
数组，不伪装成固定马点。

`ron_resolution` 初期支持：

- `head_bump`：只允许距离放铳者最近者和牌；
- `multiple`：允许多家荣和。

## 普通默认

四麻普通配置：

- 东南战、25000 点持有、30000 点返；
- 击飞、切上满贯、流局满贯、累计役满；
- 特殊双倍役满与役满叠加；
- 三赤、一发、里宝、杠宝；
- 四种途中流局；
- 流局亲家听牌连庄；
- 固定马点 `+30,+10,-10,-30`。

三麻普通配置保持 25000 初始点，使用筒、索各一赤，关闭不可能的四风连打
和四家立直，固定马点 `+30,0,-30`。

## 版本化预设

每个预设保存 `id + revision`。下表只声明本项目已经建模字段的映射，不声称
覆盖线下竞赛的裁判、犯规和器材条款。

### `jpml-a@1`

依据日本职业麻将联盟公开规则：

- 30000 点持有、30000 点返、无击飞；
- 无一发、里宝、杠宝和赤牌；
- 无途中流局；
- 无切上满贯、无累计役满；
- 役满可以叠加，特殊形不额外双倍；
- 听牌连庄；
- 官方浮动顺位点。

来源：

- https://www.ma-jan.or.jp/column_other/115081.html
- https://www.ma-jan.or.jp/activity/game_rule.html

### `saikouisen@1`

依据最高位战日本职业麻将协会 2025-08-25 规则概要：

- 30000 点持有、30000 点返、无击飞；
- 一发、里宝、杠宝，零赤；
- 无途中流局；
- 切上满贯、无累计役满；
- 役满可以叠加，特殊形不额外双倍；
- 听牌连庄；
- 固定马点 `+30,+10,-10,-30`。

来源：

- https://saikouisen.com/about/rules/
- https://saikouisen.com/pdf/saikouisen_regulation_250825.pdf.pdf

### `m-league@1`

依据 M League 官网公开规则：

- 东南战、25000 点持有、30000 点返、无击飞；
- 三赤、一发、里宝、杠宝；
- 无途中流局；
- 切上满贯、无累计役满；
- 役满可以叠加，特殊形不额外双倍；
- 听牌连庄；
- 固定马点 `+30,+10,-10,-30`，另由返点差产生 20000 点 Oka；
- 头跳。

来源：

- https://m-league.jp/about/

## 校验错误

校验一次返回全部问题，错误包含稳定机器码、字段路径和简短说明。首批错误：

```text
rules.points.out_of_range
rules.points.not_thousand_aligned
rules.red_fives.too_many
rules.sanma.red_man_five
rules.sanma.four_winds
rules.sanma.four_riichi
rules.uma.player_count
rules.uma.not_zero_sum
```

房间 API 将这些错误映射为 `request.invalid_rule_config`，并保留字段级 details。

## 快照 JSON

```json
{
  "schema_version": 1,
  "rule_set_id": "riichi/yonma",
  "engine_version": "0.1.0",
  "preset": {"id":"m-league","revision":1},
  "config": {}
}
```

快照写入解析后的完整 `config`。读取时先严格反序列化，再重新校验；不能因
来自数据库而跳过不变量检查。

