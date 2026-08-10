# ADR-0011：接入第二套规则集（冲击麻将）

状态：采纳  
日期：2026-08-08

## 背景

在冲击麻将之前，仓库里只有立直麻将一套引擎。ADR-0003 早就把边界画好了
（「规则引擎拥有玩法状态」），但边界从未被第二套规则验证过：
`GameRuleSnapshot` 只有 `Riichi` 一个分支，`GameRuntime` 直接持有
`RiichiMatch` / `RiichiHand`，`MatchRecord.rule_snapshot` 是
`RiichiRuleSnapshot` 强类型，前端 `MatchView` 的每个字段都默认按立直读。

冲击麻将和立直几乎没有交集：不能吃、只能自摸、财神百搭、起始 100 点、
另有一本可为负的「杠点」账、九种「全交」（一击清台）。硬约束是用户原话
——**「添加规则时，不要影响已经有的功能」**。

## 决定

**一、新引擎自成一个 crate。** `crates/mahjong-impact` 只依赖
`mahjong-core`，不依赖 `mahjong-riichi`，也不向 riichi 里加任何条件分支。
牌码编码（`1m`..`9s`、`1z`..`7z`）与 riichi 保持一致，纯粹是为了前端的
牌面资源路径能复用——两边的 `Tile` 类型仍然互不相干。

**二、公共层用枚举分叉，不用泛型抽象。**
`GameRuleSnapshot` / `GameRuntime` / `RoomRuleSelection` 各加一个分支。
不去抽「通用规则引擎」trait：两套规则的动作集合本身就不一样（impact 没有
吃、没有荣和、没有立直），任何能同时容纳两者的 trait 都会退化成
`serde_json::Value` 进出，反而把类型检查丢掉。

**三、投影层保持一个形状，只追加可选字段。**
`ObserverMatch` / `MatchView` 不按规则集分裂成两个 JSON schema。impact 独有
的东西（`joker_indicator` / `joker_code` / `kan_points` / `dealer_streak` /
`all_in` / `last_kan`）一律 `Option` + `skip_serializing_if`，riichi 路径下
序列化结果与改动前逐字节一致（除新增的 `variant_kind` 恒有）。前端按
`variant_kind` 分支渲染，riichi 那几片 JSX 一行不动。

已确认 `clients/`（console、bot）与 `apps/game-web/src` 中没有任何
`deny_unknown_fields`，所以追加字段不会打断既有客户端。

**四、范围限定。** 冲击麻将只开好友房、固定四人；不接入匹配队列、不接入
段位、不接入 bot。匹配队列注册处显式拒绝 `impact/*` 规则集。

## 与计划的偏差

计划里有一步「把 `GameRuntime` 中与引擎无关的会话状态抽成 `MatchShell`，
两个 runtime 各持一个」。**这一步没有做。** `ImpactRuntime` 是独立结构体，
自带一份会话生命周期代码（时钟、素材就绪、开局就绪、结算节奏、退出投票），
与 `RiichiRuntime` 重复约 400 行。

取舍：抽取是纯搬移，但它要动的是 riichi 唯一在跑的那条路径，回归风险落在
既有功能上；重复的代价则完全落在新代码里。在「不要影响已经有的功能」这条
硬约束下选了后者。**这是明确的技术债**：第三套规则进来之前应该先做这次抽取，
否则会变成三份。

## 已知缺口

- **不生成牌谱记录。** `MatchRecord.rule_snapshot` 与事件名
  （`riichi.*`）都是 riichi 强类型。归档处通过
  `Application::match_generates_record` 对 impact 直接跳过——不是失败后
  吞掉错误，而是根本不进归档路径。因此冲击麻将**没有牌谱重演**。
- **不产生事件流。** impact 的 `event_sequence` 恒为 0，
  `events_after` 返回空页。前端 game-web 走的是「视图模式」，不依赖事件流，
  所以对局本身完整可玩；但依赖事件流的客户端（console）看不到 impact 对局。
- **不接入匹配与段位。**

## 后果

- 新规则不需要碰 riichi 的任何一行，riichi 的既有测试全部原样通过；
- `MatchView` 变成一个「大部分字段可选」的联合体，读它的地方必须先看
  `variant_kind`——这个负担会随规则集数量线性增长，第三套规则时应重新评估
  是否该按规则集分裂 schema；
- 会话生命周期代码有两份，两边改动必须同步；
- 牌谱与事件流两个缺口需要在补齐前，在任何「冲击麻将已完成」的说法里明写。
