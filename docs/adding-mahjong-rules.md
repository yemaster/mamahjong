# 新麻将规则接入架构

项目现在把一场麻将拆成三层，新增规则时只需要在规则层提供麻将本身的行为。

## 分层边界

### 规则引擎

规则 crate（例如 `mahjong-riichi`、`mahjong-impact`）只负责：

- 牌组、洗牌、牌山与摸牌顺序
- 合法操作与响应优先级
- 和牌、流局、连庄和整场结束条件
- 役种、番数、符数、点数或规则专属资源
- 从一小局结果推进到下一小局

规则引擎不应该包含 React 页面、动画时长、WebSocket、素材加载或确认按钮逻辑。

### 通用对局层

`crates/mamahjong-application/src/match_flow.rs` 封装所有规则共用的 UI 握手：

- `MatchOpening`：素材加载、开局骰子/配牌动画完成上报、超时放行
- `SettlementFlow`：和牌结算动画完成上报、点数变化确认、超时推进

`crates/mamahjong-application/src/runtime.rs` 的 `RuleRuntime` 是整场对局接口，负责把任意规则接到房间、时钟、网络投影和后台定时推进。新增规则实现该接口后，应用服务不需要为每个操作继续增加枚举分派。

### 通用前端层

以下 UI 与麻将种类无关：

- `apps/game-web/src/game/useMatchOpening.ts`：素材握手、等待其他玩家、骰子、发牌
- `apps/game-web/src/game/HandSettlement.tsx`：一小局和牌/流局结算
- `apps/game-web/src/game/PointChangeOverlay.tsx`：点数变化与确认
- `apps/game-web/src/scenes/ResultScene.tsx`：整场结果

通用流程指令统一使用 `game.*`：

- `game.assets_ready`
- `game.ready_for_hand`
- `game.settlement_played`
- `game.confirm_settlement`
- `game.request_exit_vote`
- `game.vote_exit`

服务端仍接受旧的 `riichi.ready_for_hand`、`riichi.settlement_played`、`riichi.confirm_settlement`，用于兼容旧客户端。

## 新增规则步骤

假设要新增 `mahjong-guobiao`：

1. 新建规则 crate，只实现牌局、计分和局/场推进。
2. 在应用层新增 `GuobiaoRuntime`，组合规则引擎、`SeatClock`、`MatchOpening` 和 `SettlementFlow`。
3. 为 `GuobiaoRuntime` 实现 `RuleRuntime`，提供投影、命令执行和超时操作。
4. 在 `GameRuleSnapshot` 注册规则快照，并在 `GameRuntime::start` 增加一次构造映射。
5. 在服务端 DTO 把规则专属字段映射到统一的 `match_view.v1`；共用字段继续使用现有字段。
6. 前端只为真正独有的操作扩展 `ActionPanel` 或牌桌标记，不再重写开局、摸牌、结算、点数变化和结果页面。

## Runtime 最小职责

一套新规则的 Runtime 应只保留这些规则相关内容：

- 当前规则引擎的整场状态
- 当前小局状态
- 玩家到规则座位类型的转换
- 规则命令到引擎方法的映射
- 规则投影到统一牌桌视图的映射
- 规则专属动画事件，例如冲击麻将的杠点变化

不要再次复制素材数组、开局动画计时、结算播放数组、确认倒计时。它们应分别由 `MatchOpening` 和 `SettlementFlow` 管理。

## UI 数据约定

新规则应尽量填充统一字段：

- `players`、`phase`、`progress`
- `remaining_live_draws`
- `opening_ready_seats`、`assets_ready_seats`
- `hand_settlement.point_deltas`
- `hand_settlement.points_before`、`points_after`
- `result.final_points`、`result.placements`

规则独有数据使用可选字段，不要为同一种 UI 流程创建另一套页面。例如财神指示牌、杠点可以是扩展字段，但和牌后的点数动画仍使用统一结算结构。
