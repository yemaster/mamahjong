# 操作时限与断线

状态：实现完成  
最后更新：2026-08-01

## 目标

- 每个待决策的座位有明确剩余时间，超时由服务端自动推进，牌局不会卡住。
- 时限是对局状态的一部分：重连后剩余时间连续，不因断线获得额外思考时间。
- 客户端能显示「谁在思考、还剩多久」，并区分基础时间和长考时间。
- 掉线的玩家保留座位，其他玩家能看到掉线状态。
- 时间不进入事件日志，回放不依赖真实时钟。

## 现状与缺口

已有能力：

- `GameRuntime` 用 `HandPhase` 明确当前等待谁：`AwaitingTurnAction`、
  `AwaitingDiscard`、`AwaitingResponses`、`Ended`；
- 无人可响应时 `advance_automatic_reactions` 已自动推进；
- WebSocket 已能推送事件并按游标续传。

缺口：

- 没有时限，任何一个玩家不操作就永久阻塞整桌；
- 应用层没有时钟，也不应该有：领域与用例层保持同步、无 tokio；
- 客户端没有倒计时和掉线提示。

## 范围

- `crates/mamahjong-application`：座位时钟、超时动作、到期扫描。
- `apps/server`：单调时钟、到期任务、`clock` 与 `presence` 消息。
- `apps/game-web`：WebSocket 订阅与重连，显示倒计时与掉线状态。

不在本阶段：观战席时钟、托管（自动打牌）模式。

## 时间模型

参考雀魂段位战与天凤的两段式时间：

| 参数 | 默认值 | 含义 |
|---|---|---|
| 基础时间 | 5000 ms | 每次决策重置 |
| 长考时间 | 20000 ms | 单局共用一池，只减不增，下一局补满 |

创建房间可选 `5+0`、`5+20`、`5+60`、`15+60`。前项是每次决策都会
恢复的基础秒数，后项是每局开始补满的固定长考秒数。

一次决策的可用时间是「基础时间 + 该座位剩余长考时间」。决策完成时，超出
基础时间的部分从长考池扣除；未超出则长考池不变。长考池耗尽后，每次决策
只有基础时间。

选择两段式而不是单一时长：单一短时长会让复杂牌局频繁超时，单一长时长会
让整局节奏拖沓。两段式让常规打牌保持快节奏，同时给关键判断留出余量。

## 时钟归属

时间只有一个来源：服务端进程启动时刻。

```text
apps/server        MonotonicClock::now_ms()  →  u64 毫秒
mamahjong-application  接收 now_ms，不读取任何时钟
```

应用层保存的都是同一坐标系下的绝对毫秒，读接口不需要 `now`：

```rust
struct SeatClock {
    /// 该座位开始等待的时刻；不在等待时为 None。
    armed_at_ms: Option<u64>,
    /// 每次决策恢复的基础时间。
    base_ms: u64,
    /// 剩余长考时间。
    reserve_ms: u32,
}
```

`deadline_ms = armed_at_ms + base_ms + reserve_ms`。视图返回
`armed_at_ms`、`deadline_ms` 和 `reserve_ms`，由传输层换算成
`remaining_ms`。这样 `match_view` 和 `match_events` 保持纯读，不必层层
传递时间。

需要时间的入口只有三个：

```rust
GameRuntime::start(room, id, now_ms)
GameRuntime::execute(actor, command, now_ms)
Application::expire_clocks(now_ms) -> Vec<ClockExpiry>
```

## 上钟与扣时

每次事件推进后重新计算所有座位的时钟：

| 阶段 | 上钟的座位 |
|---|---|
| `AwaitingTurnAction { seat }` | 该座位 |
| `AwaitingDiscard { seat }` | 该座位 |
| `AwaitingResponses` | 尚未应答且有合法响应的座位 |
| `Ended` | 无 |

已在等待且仍需等待的座位保留原 `armed_at_ms`，不因别人的响应而回血。新
上钟的座位从当前时刻开始计时。下钟时结算：

```text
used     = now_ms - armed_at_ms
overrun  = used.saturating_sub(BASE_MS)
reserve -= min(reserve, overrun)
```

响应阶段多个座位并行计时，各自独立扣自己的长考池。

### 动画宽限

上一步操作在客户端还有动画要播：吃碰杠的横幅、副露推到副露位、打出的牌飞
到牌河。这段时间下一家看不到完整牌面，不该算进他的思考时间。所以新上钟的
`armed_at_ms` 不是「现在」，而是「现在 + 上一条指令的动画时长」——`arm()`
接受未来时刻，宽限期内该座位不读秒。

宽限时长由 `mamahjong-application::presentation::animation_grace_ms` 按指令
给出，客户端 `apps/game-web/src/game/animationTiming.ts` 有一份同名同值的
镜像；`presentation.rs` 的测试直接读那个文件逐项比对，改一边不改另一边测试
会红。`SeatCountdown::snapshot` 把未到期的宽限折进 `remaining_ms`，因此客户
端插值出来的秒数和服务端一致。

常量表与结算阶段的兜底时刻见[对局推进](../engine/match-progression.md)第五节。

## 超时动作

到期只做最保守的动作，不替玩家做收益判断：

| 阶段 | 自动动作 |
|---|---|
| `AwaitingTurnAction` | 摸切；无摸牌时打出手牌最右一张 |
| `AwaitingDiscard` | 打出手牌最右一张 |
| `AwaitingResponses` | 该座位过 |

不自动自摸、不自动荣和、不自动立直、不自动副露。错过荣和由规则引擎按振听
处理，与玩家主动过完全一致。

超时动作走与玩家命令相同的 `execute` 路径，因此产生同样的事件与序号；
客户端不需要区分事件来源。版本号照常递增，正在提交的旧版本命令会得到
`game.stale_version`。

## 到期扫描

`Application::expire_clocks(now_ms)` 遍历未结束的对局，对每个到期座位执行
超时动作，返回发生变化的对局：

```rust
pub struct ClockExpiry {
    pub match_id: MatchId,
    pub actor: UserId,
    pub version: u64,
    pub latest_sequence: u64,
    pub finished: bool,
}
```

传输层用 200 ms 周期的任务调用它，并对返回的每场对局发布 `StreamNotice`，
复用 P2 的唤醒—拉取通道。选择集中扫描而不是每场一个定时任务：对局数量在
单进程内有限，集中扫描没有任务泄漏问题，且 `expire_clocks` 是纯函数，测试
可以直接喂入任意 `now_ms`，不需要真实等待。

一次扫描内同一座位可能连续到期多次（例如超时打牌后立刻轮到自己），每场
对局每次扫描最多推进一次，剩余的到期留给下一个周期，避免单场对局在一次
扫描中跑完整局。

## 消息

`clock.v1` 在 `welcome` 之后、以及每次补发事件之后发送：

```json
{
  "kind": "clock",
  "schema": "clock.v1",
  "stream": "match_matchId",
  "version": 43,
  "server_time": "2026-08-01T12:00:00Z",
  "seats": [
    {"seat": 1, "remaining_ms": 24200, "base_ms": 4200, "reserve_ms": 20000}
  ]
}
```

- `seats` 只列出正在计时的座位，空数组表示当前无人上钟；
- `base_ms` 是本次决策剩余的基础时间，`reserve_ms` 是长考池余量，
  `remaining_ms` 是两者之和；
- 客户端收到后本地倒计时，不轮询；`pong` 的 `server_time` 用于校正漂移。

`presence.v1` 在订阅建立和连接断开时发送给该流的其他连接：

```json
{
  "kind": "presence",
  "schema": "presence.v1",
  "stream": "match_matchId",
  "seats": [{"seat": 0, "online": true}, {"seat": 1, "online": false}]
}
```

在线状态由传输层的连接表推导，不进入应用层状态：掉线不改变座位、不改变
时钟，只影响客户端展示。

## 客户端

客户端事件驱动，不在客户端重建牌局状态：

- 进入对局时建立 WebSocket，订阅 `match_{match_id}`；
- 收到 `event` 后拉取一次观察者视图，同一时刻只保留一个在途请求；
- 收到 `clock` 后本地倒计时，渲染当前思考座位与秒数；
- 收到 `presence` 后在座位上标记掉线；
- 连接断开时退回 HTTP 轮询，并按退避重连，重连时带上最后一个连续事件序号。

客户端只把事件当作「状态已变化」的信号，唯一真相仍是观察者视图。不同
客户端平台不需要各自实现一套状态归约。

## 模块

```text
crates/mamahjong-application/src/clock.rs   SeatClock、时限常量、超时动作
apps/server/src/clock.rs                    MonotonicClock、到期任务
apps/server/src/api/realtime/message.rs     clock.v1、presence.v1
apps/server/src/api/realtime/hub.rs         连接表与在线状态
apps/game-web/src/ws.ts                     WebSocket 订阅与重连
```

## 测试

应用层：

- 超时前后 `deadline_ms` 与 `reserve_ms` 的变化符合两段式扣时；
- 基础时间内完成决策不扣长考；超出只扣超出部分；长考耗尽后不为负；
- 响应阶段两个座位分别超时，各自只扣自己的长考池；
- 摸牌阶段超时打出摸到的牌；立直座位超时结果与手动摸切一致；
- 一次扫描中每场对局最多推进一次。

传输层与端到端：

- `welcome` 之后收到 `clock`，`seats` 与当前阶段一致；
- 超时后未操作的一方与旁观的一方收到同一组事件；
- 断线重连后 `remaining_ms` 连续，不因断线延长；
- 连接断开后其他连接收到 `presence`，`online` 为 false。

## 验收

- 任一玩家全程不操作，整场对局仍能自然结束。
- 超时产生的事件与手动操作产生的事件在协议上不可区分。
- 重连后牌面与剩余时间与未断线玩家一致。
- 格式化、Clippy 和全工作区测试通过。

## 参考

- [实时传输](realtime-transport.md)
- [通信协议](api.md)
- [日麻单局状态机](../engine/riichi-hand-state.md)
- [对局推进与前后端同步](../engine/match-progression.md)
