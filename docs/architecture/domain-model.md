# 对象模型

状态：M0 设计基线  
最后更新：2026-07-29

本文定义边界和不变量，不提前规定日麻内部算法。

## 限界上下文

| 上下文 | 核心对象 | 职责 |
|---|---|---|
| Identity | `User`、`Session` | 身份、认证、封禁状态 |
| Lobby | `Room`、`RoomMember` | 房间配置、成员、准备和开局 |
| Matchmaking | `QueueTicket`、`MatchProposal` | 段位队列和配桌 |
| Game | `GameMatch`、`Hand`、`RuleSnapshot` | 权威牌局状态与动作 |
| Ranking | `Rating`、`HandResult`、`MatchResult` | 每局记录、整场结算与段位 |

上下文之间只传标识符和不可变数据，不共享可变实体。

## 标识符与基础类型

- 房间使用六位随机数字房间号，便于输入和分享；其余实体使用不可猜测、
  全局唯一且带前缀的 UUIDv7，如 `match_...`。
- Rust 内部为每类 ID 建立 newtype，禁止交换 `RoomId` 与 `MatchId`。
- 金额、点数、番、符使用整数，不使用浮点数。
- 持久化时间统一为 UTC；领域规则需要“当前时间”时由应用层传入。
- 聚合使用单调递增的 `version: u64`，每个成功命令至少递增一次。

前缀不是类型判定依据，只用于日志和人工排障。

## 聚合

### User

`User` 只保存稳定身份、账号状态和档案引用。登录凭据与公开档案分离；昵称
可修改且不要求唯一。称号、段位和使用角色通过独立的版本化关联扩展，不能
把展示字符串用作牌局外键。详细约束见
[用户注册与档案](../server/identity-profile.md)。

### Room

房间只管理开局前及局间成员关系，不保存牌局内部状态。

```text
Room
├── id, version, owner_id
├── name, visibility, lifecycle
├── rule_snapshot
├── seats[seat_count]
└── spectators
```

主要不变量：

- 房主必须是成员；
- 仅房主可在等待状态修改规则、移除成员和开始；
- 修改规则会重新生成完整 `RuleSnapshot`，并清除全部准备状态；
- 一个用户最多占一个座位；
- 开局要求座位满员、所有成员准备且规则快照有效；
- 开局后房间引用 `MatchId`，不直接嵌入 `GameMatch`。

房主离开时按加入顺序转移；无人后房间进入关闭状态。进行中的桌局断线不
等价于离开。

### QueueTicket

段位匹配票固定保存用户、规则集、入队顺序和状态。首版按四麻、三麻分别
FIFO 配桌；同一用户最多有一张 `waiting` 票。人数满足时一次写操作内创建
私有房间、固定座次、生成规则快照并开始 `GameMatch`，所有入选票同时变为
`matched`。取消只允许作用于仍在等待的票。

段位、等待时长和网络质量以后作为候选筛选条件加入，不改变
`waiting → matched / cancelled` 生命周期，也不让客户端指定对手或座次。

### GameMatch

`GameMatch` 是服务端的权威游戏聚合，也是单写者并发边界。

```text
GameMatch
├── id, version, lifecycle
├── rule_snapshot
├── players (固定座次与累计分)
├── progression (场次/局次，由规则引擎解释)
├── active_hand
└── result
```

主要不变量：

- 创建后玩家、座次和规则快照不可变；
- 只有当前版本匹配的命令可以改变状态；
- 命令先通过通用校验，再由对应规则引擎判定；
- 每次转换生成一个或多个领域事件，不能静默修改；
- 已结束牌局拒绝游戏动作；
- 客户端从不提交点数、牌山、可用动作等权威结果。

### Hand

`Hand` 的具体结构由规则引擎拥有。公共层只要求它能够：

- 从已验证的规则快照和随机源创建；
- 接受带玩家身份的规则命令；
- 产生确定的状态转换和领域事件；
- 生成按观察者裁剪后的视图；
- 序列化为带版本的内部快照。

这避免公共层假定麻将一定有 34 种牌、四个方位或日麻式牌河。

每个 `Hand` 结束后生成不可变 `HandResult`。它至少关联起止事件序号、规则
引擎给出的局标识、庄家、结束原因和各玩家点数变化。`GameMatch` 结束后生成
`MatchResult`，保存整场座次、最终点数、排名与规则快照。东风战、半庄和将来
其他赛制都使用 `GameMatch`，不以“半庄”作为存储层硬编码类型。

## 规则对象

### RuleSetId

规则实现使用稳定、带命名空间的 ID：

```text
riichi/yonma
riichi/sanma
sichuan/bloody-battle
wuhan/classic
```

`RuleSetId` 只选择引擎；配置决定引擎允许的细节。

### RuleSnapshot

房间和牌局保存解析后的完整快照，不只保存预设名：

```json
{
  "schema_version": 1,
  "rule_set_id": "riichi/yonma",
  "engine_version": "0.1.0",
  "preset": {
    "id": "m-league",
    "revision": 1
  },
  "config": {}
}
```

- `schema_version` 控制快照外壳；
- `engine_version` 选择可确定重放的规则实现；
- `preset` 仅用于展示和来源追踪；
- `config` 是解析默认值、覆盖项后的完整配置。

进行中的牌局不随服务器默认值或预设更新而变化。

### RuleEngine

应用层通过注册表按 `RuleSetId + engine_version` 找到引擎。接口能力分为：

1. 返回元数据和可供 UI 生成表单的配置描述；
2. 解析、补全并校验配置；
3. 创建确定性的初始桌局；
4. 解码并处理命名空间内的命令；
5. 从内部状态生成指定观察者的视图；
6. 迁移该引擎旧版本的快照。

引擎内部可以使用紧凑的牌索引优化性能；外部不依赖该索引。

## 命令、事件与视图

```text
Client intent
    ↓
Application command
    ↓ validate identity, idempotency, expected version
Rule engine transition
    ↓
Domain events
    ├── durable event log
    ├── new snapshot
    └── observer-specific wire events/views
```

- 命令表达意图，例如“打出这张牌”，不表达结果。
- 领域事件记录已经发生的事实，具有稳定名称和独立版本。
- 网络事件是领域事件的投影，不能直接序列化内部事件。
- 视图分为本人、对手、观战和牌局结束后的复盘视图。
- 任何隐藏信息必须采用默认拒绝策略：未明确允许即不输出。

## 生命周期

```text
Room:  Waiting ──start──> Playing ──match ended──> Waiting
          └────close──────────────────────────────> Closed

Match: Created ──deal──> Running ──rules decide──> Finished
                          └──fatal recovery───────> Suspended
```

`Suspended` 只用于无法安全自动恢复的系统故障，不用于普通断线。
