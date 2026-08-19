# 四川麻将（血战到底）设计

第三套规则。沿用 `docs/adding-mahjong-rules.md` 的三层架构：规则引擎 crate → 应用层 RuleRuntime → 通用前端。素材数组、开局动画计时、结算播放数组、确认倒计时**不复制**，仍由 `MatchOpening` / `SettlementFlow` 提供。

## 规则

### 牌与墙
- 只用万筒索，108 张（27 种 × 4），无字牌。
- 墙：东西各 14 墩、南北各 13 墩（共 54 墩）。
- 开牌：庄家为 1，逆时针数到骰点之和，留出「较小骰点」墩后开摸（与冲击麻将一致）。
- 无翻宝牌 / 财神。

### 骰子与换三张方向
开局掷两骰，点数之和同时决定开牌与换三张方向。顺/逆时针时四家沿整圈传递，对家时两两交换：

| 骰和 | 方向 | 传递路径 |
|---|---|---|
| 2、6、10 | 逆时针（下家） | 0→1→2→3→0 |
| 4、8、12 | 顺时针（上家） | 0→3→2→1→0 |
| 3、5、7、9、11 | 对家 | 0↔2、1↔3 |

### 换三张
发牌后、定缺前。每家选 3 张同花色牌，沿骰子决定的方向传给接收家。超时则随机选 3 张同花色。

### 定缺
换三张后，每家从万/筒/索选一门定缺，头像右下角大字徽章显示。规则：
- 手上有定缺门牌时，只能打定缺门牌（强制优先打缺）。
- 不能碰 / 明杠定缺门牌；可暗杠。
- 胡牌时牌型不得含定缺门（花猪不能胡）。
- 流局时牌型含定缺门 → 查花猪赔付。

### 血战到底
- 每家初始 0 分。
- 一家胡后不结束，胡者盖牌退出，其余继续，直到 3 家胡或牌山摸尽。
- 胡者下家继续摸打。

### 番型（基础取最高，不叠加；加番叠加）
| 番型 | 番 | 说明 |
|---|---|---|
| 平胡 | 1 | 普通胡 |
| 对对胡 | 2 | 四副刻/杠 + 将 |
| 清一色 | 3 | 同一花色 |
| 七对 | 3 | 七个对子 |
| 清对 | 4 | 清一色 + 对对胡 |
| 龙七对 | 5 | 七对含四张相同（1 根） |
| 清七对 | 5 | 清一色 + 七对 |
| 天胡 / 地胡 | 6 | 庄 / 闲家第一巡自摸 |

加番：自摸 +1、根每根 +1、杠上花 +1、杠上炮 +1、抢杠胡 +1、金钩钓 +1、海底 +1。

分数 = 2^(番−1)，封顶 6 番（32 分）。

### 杠（雨）
- 暗杠：其余三家各付 2 分。
- 明杠（直杠）：放杠者付 2 分。
- 加杠（巴杠）：其余三家各付 1 分。

杠与胡都即时触发点数动画（复用冲击麻将的点数动画）。

### 流局
牌山摸尽未满 3 家胡 → 查花猪 + 查大叫：
- 查花猪：手牌含三门者赔其余三家各 8 分（封顶番的底）。
- 查大叫：未听牌者赔每位听牌者 1 番（1 分）。

流局先展示各未胡者听/不听，再播点数动画，随后按胡家顺序逐家播结算页。

### 局制
4 局。首局庄 = 东；之后庄 = 上一局第一个胡者。4 局打完结算。

## 引擎 crate：`crates/mahjong-sichuan`

镜像 `mahjong-impact` 结构：
- `tile.rs` — 万筒索 27 种、`Suit`、`TileKind`（复用 impact 编码 1m..9s，去掉字牌校验）。
- `wall.rs` — 14/13 墩墙、`Dice`、`WallSeed`、无财神。
- `hand/model.rs` — `Meld`、`Discard`、`HandPhase`（增 `AwaitingExchange` / `AwaitingDingQue`）。
- `hand/state.rs` — 摸打状态机：定缺约束、胡牌判定、杠（含抢杠）、流局。
- `scoring.rs` — 番型判定 + 分数（`evaluate` 返回番型表、番、分）。
- `match_state.rs` — 血战到底：多胡家、查花猪/查大叫、4 局制、`SichuanMatch`。
- `config.rs` / `definition.rs` / `snapshot.rs` / `preset.rs` / `overrides.rs` / `validation.rs` — 镜像 impact；`thinking_time` 可调，其余写死。

## 应用层：可复用流程 + `SichuanRuntime`

新增两个可复用阶段（与 `MatchOpening` / `SettlementFlow` 同级），供未来带换牌/定缺的麻将直接复用：

- **`ExchangeFlow`** — 抽象「每家选 N 张同花色 → 按方向置换 → 超时随机 → 播完动画报告 → 全完成/超时推进」。方向置换、可选牌集、N、超时参数化。
- **`DingQueFlow`** — 抽象「每家选一门 → 超时兜底 → 全完成推进」。

`SichuanRuntime` 组合 `MatchOpening` + `ExchangeFlow` + `DingQueFlow` + `SichuanMatch` + `SeatClock` + `SettlementFlow`，实现 `RuleRuntime`。新增 `GameCommand`（`sichuan.exchange`、`sichuan.ding_que`、`sichuan.discard`、`sichuan.tsumo`、`sichuan.ron`、`sichuan.pon`、`sichuan.open_kan`、`sichuan.concealed_kan`、`sichuan.added_kan`、`sichuan.pass`、`sichuan.exchange_animation_played` 等）。

## 后端接线
- `room.rs`：`GameRuleSnapshot::Sichuan`。
- `service.rs`：`RoomRuleSelection::Sichuan` + `resolve_rules` 分支。
- `runtime.rs`：`MatchProjection::Sichuan` + `GameRuntime::start` 分派。
- `api/rules.rs`：`sichuan/yonma` 目录项（family `sichuan`，display_name `四川麻将`）。
- `api/dto.rs`：`variant_kind = "sichuan"` 映射，新增 `exchange_direction`、`que_suits`（各座定缺门）、胡家盖牌、点数变动等字段。
- `naming.rs` / `presentation.rs`：新增 `sichuan_yaku_name`、动画时序表项。

## 前端
- `types.ts`：`MahjongFamily` 增 `sichuan`，新增 `SichuanRuleConfig`、`que_suits`、`exchange_direction`、`exchange`/`ding_que` 阶段字段。
- `ruleTitle.ts`：`sichuan: 四川麻将`。
- `CreateRoomPanel.tsx`：四川麻将仅「思考秒数」一项。
- `MatchHud.tsx`：定缺徽章（头像右下角，约 32px 大字）。
- 换三张：大字方向动画 + 选牌面板；定缺：选花色面板。
- 胡牌 / 自摸：胡的那张牌变浅红，胡家手牌盖住（血战到底继续时不可见），只在游戏结束逐家结算时摊开。
- 点数动画：复用 `PointChangeOverlay`；杠复用 `KanPointOverlay` 同款。
- 帮助页：`sichuanReferenceData.ts`（番型表）。

## 兼容性
`variant_kind` 判别字段贯穿前后端；新增 `sichuan` 只增分支，不改 riichi / impact 现有行为。动画时序新增常量需同步 `animationTiming.ts`（有测试强制一致性）。

## 实施顺序
1. 头像/名字增大（已完成）。
2. `mahjong-sichuan` 引擎（含单元测试）。
3. 应用层 `ExchangeFlow` / `DingQueFlow` + `SichuanRuntime` + 后端接线 + DTO。
4. 前端类型、建房、牌桌、徽章、动画、帮助页。
5. `cargo build/test` + 前端 build/test 全量回归。
