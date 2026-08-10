# 实时传输

状态：服务端已实现  
最后更新：2026-08-01

## 目标

- 牌局事件由服务端主动推送，客户端不再轮询 `GET /api/v1/matches/{id}`。
- 牌局命令可以走同一条连接提交，并得到明确的成功或失败回执。
- 连接断开后可以按事件序号续传，不丢事件、不重复应用。
- 事件按观察者裁剪，任何人都拿不到别人的手牌和牌山。
- 应用层和领域层不引入传输依赖，WebSocket 只存在于传输层。

## 现状与缺口

已有能力：

- `GameRuntime` 保存整场的 `Vec<GameEventRecord>`，序号 `sequence` 从 1
  单调递增，覆盖全部 17 种 `HandEvent`；
- `record.rs` 已把每个 `HandEvent` 映射为 `riichi.*` 事件名和 JSON payload，
  归档记录复用同一套结构；
- `ObserverMatch` 已按观察者裁剪手牌，并带 `version` 和 `event_sequence`；
- `AuthenticatedUser` 已从 `Authorization: Bearer` 解析会话。

缺口：

- 没有 WebSocket 路由，`POST /api/v1/ws-tickets` 仍是规划资源；
- 事件只在整场归档时对外可见，进行中的对局无法按游标读取；
- 应用层没有“取第 N 号之后的事件”接口；
- 事件 payload 只有归档视角，没有按观察者裁剪的版本；
- 客户端只能靠反复请求观察者视图发现变化。

## 范围

本阶段实现：

- 一次性 ws ticket 的签发与消费；
- WebSocket 建连、`hello` / `welcome`；
- 命令、命令回执、事件、错误四种信封；
- 按观察者裁剪的事件读取与游标续传；
- 心跳与基本流控。

本阶段不实现（属于 P3 及以后）：

- 操作倒计时和超时自动过；
- 房间大厅与匹配队列的实时推送；
- 观战者订阅；
- 跨进程广播（当前为单进程内存实现）。

## 流

流名与协议文档一致，形如：

```text
match_{match_id}
```

首版只有牌局流。`hello` 中出现其他前缀一律拒绝，错误码
`request.unknown_stream`。订阅者必须是该场对局的玩家：应用层
`seat_for` 对非玩家返回 `NotMatchPlayer`，传输层据此拒绝订阅，
错误码 `auth.forbidden_stream`。

一条连接可以订阅多个流。首版同一用户可以建立多条连接，同一流的多条连接
各自独立收事件，服务端不做互踢。

## Ticket

长期访问令牌不进入 URL，也不进入服务端访问日志，因此建连前先换取一次性
ticket：

```text
POST /api/v1/ws-tickets
Authorization: Bearer <session_token>
```

响应：

```json
{
  "schema": "ws_ticket.v1",
  "ticket": "opaque",
  "expires_in": 30
}
```

约束：

- ticket 为 32 字节 CSPRNG 随机值的十六进制串，复用管理端会话的生成方式；
- 生存期 30 秒，一次消费后立即删除；
- 只绑定用户 ID，不绑定流，流由 `hello` 决定；
- 每次读取顺带清理过期项，避免额外后台任务；
- 单用户同时最多持有 8 张未消费 ticket，超出时淘汰最早一张。

建连：

```text
GET /api/v1/ws?ticket=<opaque>
```

ticket 无效或过期返回 `401` 与 `auth.invalid_ticket`，不进入 WebSocket
升级。升级成功后连接身份固定，后续消息不再接受身份声明。

## 消息

### 客户端 → 服务端

`hello` 必须是第一条消息，否则连接以 `request.expected_hello` 关闭。

```json
{
  "kind": "hello",
  "protocol": "mamahjong.v1",
  "subscriptions": [
    {"stream": "match_01J...", "after_seq": 120}
  ]
}
```

- `after_seq` 省略或为 `0` 表示从头补发；
- `subscriptions` 最多 4 项，重复流名视为格式错误。

命令沿用协议文档的 `command.v1`：

```json
{
  "kind": "command",
  "schema": "command.v1",
  "command_id": "cmd_opaque",
  "stream": "match_01J...",
  "expected_version": 42,
  "name": "riichi.discard",
  "payload": {"tile_id": 37}
}
```

`name` 与 `payload` 的组合与 HTTP 命令完全一致，共用同一个反序列化枚举，
不出现两套命令定义。

心跳：

```json
{"kind": "ping"}
```

### 服务端 → 客户端

`welcome` 在补发历史事件之前发送：

```json
{
  "kind": "welcome",
  "schema": "welcome.v1",
  "connection_id": "conn_opaque",
  "protocol": "mamahjong.v1",
  "heartbeat_interval": 20,
  "streams": [
    {"stream": "match_01J...", "version": 43, "event_seq": 121}
  ]
}
```

`streams[].event_seq` 是订阅时服务端已有的最新序号；客户端据此判断还要
等待多少补发事件。

事件、命令回执和错误直接使用协议文档的 `event.v1`、`command_result.v1`
和 `error.v1`，不新增字段。事件额外带 `hand_index`，便于客户端按小局分段：

```json
{
  "kind": "event",
  "schema": "event.v1",
  "stream": "match_01J...",
  "seq": 121,
  "version": 43,
  "hand_index": 2,
  "name": "riichi.tile_discarded",
  "payload_schema": 1,
  "payload": {"seat": 1, "tile": {}, "tsumogiri": false, "riichi_declared": false}
}
```

心跳回复：

```json
{"kind": "pong", "server_time": "2026-07-31T12:00:00Z", "latest_seq": 121}
```

## 事件裁剪

事件序号对所有观察者一致：同一 `seq` 在不同连接上是同一件事，只是 payload
详略不同。这样断线续传的游标语义与观察者无关。

17 种 `HandEvent` 中只有三种含隐藏信息：

| 事件 | 本人 | 他人 |
|---|---|---|
| `riichi.initial_hand_dealt` | 完整 `tiles` | 去掉 `tiles`，改为 `tile_count` |
| `riichi.tile_drawn` | 完整 `tile` | 去掉 `tile`，保留 `seat`、`source`、`remaining_live_draws` |
| `riichi.furiten_changed` | 完整字段 | 只保留 `seat` |

其余事件都是公开事实（打牌、副露、宝牌指示牌、立直、和牌、流局），原样
下发。牌山、随机种子和规则引擎内部状态在任何视角下都不出现。

裁剪在应用层完成，传输层拿到的已经是可直接序列化的结果，避免传输层重新
理解领域语义。

## 应用层接口

`Application` 增加只读接口：

```rust
pub fn match_events(
    &self,
    actor: &UserId,
    match_id: &MatchId,
    after_sequence: u64,
) -> Result<MatchEventPage, ApplicationError>;
```

```rust
pub struct MatchEventPage {
    version: u64,
    latest_sequence: u64,
    events: Box<[MatchEvent]>,
}

pub struct MatchEvent {
    sequence: u64,
    hand_index: u32,
    name: &'static str,
    event_version: u8,
    payload: serde_json::Value,
}
```

行为：

- 非本场玩家返回 `NotMatchPlayer`；
- 只返回 `sequence > after_sequence` 的事件，按序号升序；
- payload 已按 `actor` 裁剪；
- 单次最多返回 512 条，`latest_sequence` 始终是当前最新序号，客户端据此
  判断是否需要继续拉取。

`event_payload` 由 `record.rs` 提升为 crate 内共享函数，归档记录与实时事件
共用同一份名称和字段定义，避免两处漂移。

## 推送实现

推送采用“唤醒 + 拉取”：

1. 传输层持有 `RealtimeHub`，内部是
   `Mutex<HashMap<String, broadcast::Sender<StreamNotice>>>`；
2. 服务端每次修改对局或房间后调用 `hub.publish(stream, notice)`；
3. `StreamNotice` 只有 `{version, latest_sequence}`，是信号不是数据；
4. 连接收到信号后调用 `match_events(actor, match_id, cursor)` 拉取并下发。

这样做的原因：

- 广播通道里不放事件本体，就不需要为每个观察者维护一份裁剪后的副本；
- `RecvError::Lagged` 无害，落后只意味着少收几次信号，随后一次拉取即可
  追平，不必关闭连接；
- 事件的唯一真相仍是 `GameRuntime.events`，续传和实时推送走同一条路径；
- 单进程实现替换为跨进程消息总线时，只需替换信号来源。

连接循环用 `tokio::select!` 同时等待客户端消息和流信号。收到客户端命令时
调用现有 `submit_game_command`，成功后立即发布信号并回 `command_result`；
失败时回 `error.v1`，不关闭连接。

发布点集中在传输层，应用层不感知 hub：

| 发布点 | 流 |
|---|---|
| `POST /api/v1/matches/{id}/commands` | 该场对局 |
| WebSocket 命令 | 该场对局 |

两条发布点共用 `matches::apply_command`，因此 HTTP 与 WebSocket 产生的事件
序号完全一致。开局时该流尚无订阅者，不需要发布信号。房间流留待 P3，本阶段
房间操作不发布信号。

## 续传

客户端在 `hello` 中给出每个流的 `after_seq`：

- 服务端从该序号之后补发，补发完成后进入实时推送；
- 因为整场事件常驻内存，首版不存在“游标过旧”，协议中的快照回退分支保留
  但暂不触发；
- 客户端收到断号时停止提交命令，重新建连并带上最后一个连续序号；
- 命令通过 `expected_version` 防止过期界面误操作，重连不需要命令去重表。

## 观察者视图增量同步

### 为什么要有这一层

事件帧本身已经是增量的，但客户端拿它只当一个「有事发生」的信号，随后再
`GET /api/v1/matches/{id}` 拉一整份观察者视图。一份中盘视图约 `7 KB`，其中
八成五是 `players`，而里面绝大部分（用户编号、昵称、头像路径、立绘路径、
已经摆在桌上的牌河和副露）每次都一模一样。一小局有上百次推进，等于把同一
份数据重复搬运上百遍。

真正的增量不能交给客户端自己算：`turn_actions`、`available_reactions`、
`waiting_tiles`、振听这些字段是规则引擎算出来的，客户端要自己维护就得把
一整套日麻规则再实现一遍。所以增量在服务端做——按连接记住上一次发出去的
那份视图，之后只发它和新视图之间的差。

### 帧

订阅时声明要视图同步：

```json
{
  "kind": "hello",
  "protocol": "mamahjong.v1",
  "subscriptions": [
    {"stream": "match_01J...", "after_seq": 120, "view_patches": true}
  ]
}
```

- `view_patches` 缺省为 `false`，老客户端行为完全不变；
- 声明为 `true` 的订阅**不再收到 `event` 帧**：视图就是完整真相，事件帧对
  这类客户端是纯粹的重复流量。游标仍然照常推进，重连时照样能报出
  `after_seq`。

第一份是整份视图：

```json
{
  "kind": "view_snapshot",
  "schema": "view_snapshot.v1",
  "stream": "match_01J...",
  "version": 43,
  "view": { }
}
```

之后每次推进只发差：

```json
{
  "kind": "view_patch",
  "schema": "view_patch.v1",
  "stream": "match_01J...",
  "base_version": 43,
  "version": 44,
  "ops": { }
}
```

快照在三种情况下发出：订阅建立时、断线重连后（新连接没有上一份视图）、
客户端主动请求重同步时。除此之外一律是补丁。

### 顺序

顺序由三样东西一起保证，缺一不可：

- 同一条 WebSocket 上的帧本身有序，服务端的发送又都在同一个连接任务里，
  不会交错；
- 每个补丁写明它是从哪个版本推出来的（`base_version`）。客户端手上的版本
  与之不符就**不许应用**——宁可请求重来，也不能把补丁打在错误的底子上；
- 一次推进内的帧顺序固定为：视图（快照或补丁）→ `clock` → `presence`。
  倒计时和在线状态都是视图的附属信息，必须在视图之后。

对不上时客户端沿同一条连接请求重同步：

```json
{"kind": "resync", "stream": "match_01J..."}
```

服务端丢掉这条订阅记着的上一份视图，改发一份快照。之所以不让客户端退回去
走 HTTP，是因为 HTTP 响应和连接上的补丁会赛跑：拉回来的可能是更旧的一份，
反而把界面推回过去。走同一条连接就没有这个问题。

补丁只认 `base_version` 这一条链，不能再额外加一条「版本必须变大才应用」的
规则：倒计时那几个按时刻算出来的字段会让同一个版本上连着出好几个补丁，一旦
丢掉其中一个，服务端记着的底子就和客户端手上的对不上了，之后每一个补丁都打
在错的底子上，而两边都察觉不到。快照是自足的，收到就整份替换。

真正需要比版本的是 HTTP 那一路：首屏和断线轮询拉回来的整份视图可能比连接上
已经推进过的更旧，只有比手上这份新才允许覆盖。

### 补丁的写法

补丁是一棵和视图同构的树，每个节点三选一：

```text
{"set": <值>}                                   整个子树换成这个值
{"obj": {"<键>": <补丁>}, "del": ["<键>"]}       对象逐键，del 是被删掉的键
{"arr": {"len": <n>, "at": {"<下标>": <补丁>}, "push": [<值>]}}
```

数组按下标对齐：公共部分逐个求差放进 `at`，变长的部分放进 `push`，变短时
由 `len` 截断。牌河加一张牌因此只发一个 `push`，其余牌一个字节都不重发。

三条约束：

- 完全没变的子树不出现在补丁里，一层层向上省略；整份视图没变就一帧都不发。
- 补丁只描述差异，不描述语义。服务端不去判断「这是一次打牌」，客户端也不
  按事件类型分支——两边只有一套通用的树比较和树应用，规则引擎的知识不外泄
  到传输层。
- 服务端记住的那份视图，必须是它**真正发出去过**的那一份。先发帧后记，发送
  失败就不记，否则连接一抖就会在错误的底子上继续算差。

### 记在哪

上一份视图挂在连接的每条订阅上，不挂在对局上：视图是按观察者裁剪过的，四家
各不相同；顺序也是按连接保证的。一条订阅多存一份约 `7 KB` 的 JSON，代价可以
接受。

### 效果

一次打牌真正变化的是：版本号、事件序号、阶段、打牌那家的牌河与暗手、
`turn_actions`、`available_reactions`、四家倒计时。补丁在 `1 KB` 以内，
相对整份视图省掉八成以上，且随着牌河变长省得越多——整份视图会越来越大，
补丁不会。

### 时间字段

`clocks`、`exit_vote.remaining_ms` 和 `hand_settlement.confirm_remaining_ms`
是按当前时刻算出来的，每次序列化都不一样，因此补丁几乎不会真的为空。这没有
关系：它们都很小，而且客户端本来就要靠它们对齐倒计时。

## 流控

- 单条消息上限 16 KiB，超出关闭连接，错误码 `request.message_too_large`；
- 每连接每秒最多 20 条客户端消息，超出关闭连接，错误码
  `request.rate_limited`；
- 服务端每 20 秒发送一次 WebSocket ping 帧；60 秒内没有任何客户端消息则
  关闭连接；
- 发送缓冲阻塞超过 5 秒判定为慢连接并关闭，由客户端重连续传；
- 牌局事件不丢弃，只丢弃可重建的信号。

## 错误码

沿用协议文档的类别前缀，本阶段新增：

```text
auth.invalid_ticket             ticket 无效、过期或已消费
auth.forbidden_stream           不是该流的合法观察者
request.expected_hello          首条消息不是 hello，或重复发送 hello
request.unsupported_protocol    客户端协议版本不受支持
request.unknown_stream          流名不受支持、重复订阅或未订阅该流
request.unknown_kind            消息 kind 不受支持
request.too_many_subscriptions  订阅数超过上限
request.message_too_large       单条消息超限
request.rate_limited            客户端消息频率超限
```

握手和流控错误发出错误信封后关闭连接；`request.unknown_kind`、
`request.invalid_json` 和命令错误只回信封，连接保持。游戏命令错误继续复用
`game.*`，与 HTTP 使用同一套 `ApplicationError` 映射，客户端不需要分辨来源。

## 模块

```text
apps/server/src/api/realtime.rs          路由、握手、连接循环
apps/server/src/api/realtime/hub.rs      RealtimeHub、StreamNotice
apps/server/src/api/realtime/message.rs  客户端与服务端信封
apps/server/src/api/realtime/ticket.rs   WsTickets
crates/mamahjong-application/src/stream.rs  MatchEvent、裁剪
```

`AppState` 增加 `realtime: RealtimeHub` 和 `ws_tickets: WsTickets`，与
`admin_sessions` 同级。

依赖变更：

- workspace `tokio` 增加 `sync` 特性；
- `apps/server` 的 `axum` 启用 `ws` 特性；
- `apps/server` 增加开发依赖 `tokio-tungstenite` 和 `futures-util`，用于端到端
  测试；`tower::ServiceExt::oneshot` 不能完成协议升级，测试需要真实监听端口。

## 测试

应用层：

- 非玩家读取事件被拒绝；
- 游标过滤正确，`after_sequence` 等于最新序号时返回空；
- 三种含隐藏信息的事件对本人和他人产生不同 payload，且序号一致；
- 公开事件在所有视角下字节一致。

传输层：

- ticket 一次性、过期失效、跨用户不可用；
- 缺少或非法 ticket 不进入升级；
- 首条消息不是 `hello` 时按约定错误码关闭；
- 订阅非本人对局被拒绝；
- 端到端：两名玩家建连、打牌、双方按序收到事件，且只有打牌者看到自己
  摸到的牌；
- 断线重连带 `after_seq` 后不重复、不丢事件；
- 超大消息和超频消息按约定关闭。

## 验收

- 任一玩家断线重连后，牌面与未断线玩家完全一致。
- 抓包中不出现他人手牌、牌山和种子。
- HTTP 命令路径与 WebSocket 命令路径产生同一套事件序号。
- 格式化、Clippy 和全工作区测试通过。

客户端只在建连时拉一次观察者视图、其余状态由事件驱动，属于客户端改造，
随 P3 的操作倒计时一起落地。

## 参考

- [通信协议](api.md)
- [后端运行骨架](../architecture/server-runtime.md)
- [日麻单局状态机](../engine/riichi-hand-state.md)
- [axum WebSocket 示例](https://github.com/tokio-rs/axum/blob/main/examples/websockets/src/main.rs)
- [tokio broadcast](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)
