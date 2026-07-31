# 通信 API 设计

状态：HTTP 可玩版已实现
最后更新：2026-07-31

## 边界

- HTTPS：身份、规则目录、房间、匹配和历史查询。
- WebSocket：房间实时状态、牌局命令、事件、心跳和断线续传。
- JSON 是首个协议编码；外壳允许以后协商其他编码。
- 客户端协议 DTO 与领域对象、数据库对象分别定义。

基础路径为 `/api/v1`。URL 主版本只在整体语义破坏时升级；消息 payload
还有独立的 `schema` 版本。

## 通用约定

- ID 是不透明字符串，客户端不得解析。
- 分数和序号是 JSON 整数。
- 时间使用 RFC 3339 UTC 字符串。
- 可选字段缺失表示“未提供”，不使用 `null` 表示默认值。
- 未知的非关键响应字段应忽略。
- 错误使用稳定机器码，文字只用于展示。
- 目标协议中的写请求通过 `Idempotency-Key` 或消息 `command_id` 幂等；
  当前 HTTP 可玩版先使用资源版本防止重复状态推进。

## 已实现的 HTTP 资源

```text
POST   /api/v1/registrations
POST   /api/v1/sessions
GET    /api/v1/users/me
PATCH  /api/v1/users/me/profile

POST   /api/v1/rooms
GET    /api/v1/rooms
GET    /api/v1/rooms/{room_id}
PATCH  /api/v1/rooms/{room_id}
POST   /api/v1/rooms/{room_id}/members
DELETE /api/v1/rooms/{room_id}/members
PUT    /api/v1/rooms/{room_id}/members/me/readiness
POST   /api/v1/rooms/{room_id}/matches

GET    /api/v1/matches/{match_id}
POST   /api/v1/matches/{match_id}/commands
GET    /api/v1/matches/{match_id}/record

POST   /api/v1/matchmaking-tickets
GET    /api/v1/matchmaking-tickets/{ticket_id}
DELETE /api/v1/matchmaking-tickets/{ticket_id}
```

终端客户端使用 HTTP 命令并轮询观察者视图。

`match_view.v1` 的 `available_reactions` 只包含当前观察者实际可执行的响应，
不会泄露其他玩家的候选动作。例如：

```json
{
  "available_reactions": [
    {"kind": "pon", "tile_ids": [41, 42]},
    {"kind": "pon", "tile_ids": [41, 43]}
  ]
}
```

牌张 ID 用于区分赤牌等同种牌实例。空数组表示当前观察者无需响应；若所有
玩家的数组均为空，服务端在弃牌命令内直接推进到下一家摸牌，不等待客户端
提交 `riichi.pass`。存在候选动作的玩家才可提交对应响应或
`riichi.pass`。

`turn_actions` 只在当前观察者的摸牌阶段给出合法特殊动作。普通打牌在该
阶段始终成立，不重复列出。立直、暗杠和加杠同时返回可提交的牌张 ID：

```json
{
  "turn_actions": {
    "can_tsumo": false,
    "riichi_discard_tile_ids": [17, 23],
    "concealed_kan_tile_ids": [],
    "added_kan_options": [],
    "can_nine_terminals": false
  }
}
```

## 规划资源

```text
POST   /api/v1/ws-tickets
GET    /api/v1/rule-sets
GET    /api/v1/rule-sets/{rule_set_id}
POST   /api/v1/rule-sets/{rule_set_id}/validate
GET    /api/v1/matches/{match_id}/result
GET    /api/v1/matches/{match_id}/hands
GET    /api/v1/matches/{match_id}/hands/{hand_id}
GET    /api/v1/matches/{match_id}/hands/{hand_id}/replay
GET    /api/v1/matches/{match_id}/replay
```

WebSocket ticket、事件续传和历史复盘子资源尚未实现。

段位匹配首版按 `rule_set_id` 分成四麻、三麻两条 FIFO 队列。同一用户同时
只能持有一张等待中的票；人数满足时，服务端原子建立私有房间、固定座次并
开始对局。票状态为 `waiting / matched / cancelled`。段位区间、等待扩圈和
赛季积分只改变配对选择，不改变票和对局协议。

`POST /api/v1/registrations` 接收 `login_name + password + nickname`。成功
响应返回用户档案和会话；密码及密码哈希永不进入响应、领域事件或日志。
档案从首版保留 `equipped_title`、`selected_character` 和 `ranks` 容器，
具体约束见 [用户注册与档案](identity-profile.md)。

创建房间时客户端可以提交 `preset + overrides`；响应始终返回解析后的完整
`rule_snapshot`。服务端拒绝未知字段，防止拼写错误被静默忽略。

规则输入的首个稳定结构：

```json
{
  "preset": {"id": "m-league", "revision": 1},
  "overrides": {
    "match_rules": {"tobi": true},
    "scoring": {"old_yaku": false},
    "bonuses": {"red_fives": {"man": 1, "pin": 1, "sou": 1}}
  }
}
```

未提供 `preset` 时使用目标规则集的普通默认值。预设和覆盖项先解析为完整
规则并整体校验，成功后才生成不可变快照。

低频房间写操作使用当前 `version` 做条件更新。冲突返回 `409` 和最新资源
版本。

整场记录返回当时的完整规则快照、按序单局摘要和已结算小局的完整事件。
单局复盘接口只读取该局起止事件范围；整场复盘按 `hand_index` 串联所有
单局。当前正在进行的小局不进入记录响应，避免泄露隐藏信息。

`GET /api/v1/matches/{match_id}/record` 当前仅允许本场玩家读取，返回
`match_record.v1`。服务端在开局、每局结算和整场结束时把同一结构写入持久
归档。

## 当前 HTTP 牌局命令

```json
{
  "expected_version": 12,
  "command": {
    "name": "riichi.discard",
    "payload": {"tile_id": 37}
  }
}
```

无参数命令省略 `payload`。当前命令名：

```text
riichi.discard             riichi.riichi_discard
riichi.tsumo               riichi.ron
riichi.pass                riichi.nine_terminals
riichi.chi                 riichi.pon
riichi.open_kan            riichi.concealed_kan
riichi.added_kan
```

吃、碰和杠使用当前观察者手牌中的牌张 ID；加杠同时提交副露 ID。服务端根据
会话确定玩家身份，并验证版本、阶段、座位和动作。

## WebSocket 建连

客户端先通过 HTTPS 获取一次性短期 ticket，再连接：

```text
GET /api/v1/ws?ticket=<opaque>
```

长期访问令牌不放入 URL。连接建立后客户端发送：

```json
{
  "kind": "hello",
  "protocol": "mamahjong.v1",
  "connection_id": "optional_previous_connection",
  "subscriptions": [
    {
      "stream": "match_matchId",
      "after_seq": 120
    }
  ]
}
```

服务端返回 `welcome`，然后补发保留范围内的事件；若游标过旧则发送经过
权限裁剪的最新视图及其序号。

## 命令信封

```json
{
  "kind": "command",
  "schema": "command.v1",
  "command_id": "cmd_opaque",
  "stream": "match_matchId",
  "expected_version": 42,
  "name": "riichi.discard",
  "payload": {
    "tile_instance_id": "tile_opaque"
  }
}
```

- `command_id` 在用户和 stream 范围内唯一；
- `expected_version` 防止过期界面误操作；
- `name` 必须属于当前规则引擎的命名空间；
- 玩家身份从连接会话取得，不接受 payload 声明；
- 客户端只传选择，合法动作及结果由服务端计算。

成功接收不代表规则动作成功。服务端明确回复：

```json
{
  "kind": "command_result",
  "schema": "command_result.v1",
  "command_id": "cmd_opaque",
  "status": "applied",
  "version": 43,
  "event_seq": [121, 122]
}
```

## 事件信封

```json
{
  "kind": "event",
  "schema": "event.v1",
  "stream": "match_matchId",
  "seq": 121,
  "version": 43,
  "name": "riichi.tile_discarded",
  "payload_schema": 1,
  "payload": {}
}
```

同一 stream 严格按 `seq` 有序。断号时客户端暂停提交游戏命令并请求续传。
事件 payload 按观察者生成：本人、对手和观战者收到的内容可以不同，但
共享 seq 和事实名称。

不允许在 payload 中发送以下内部数据：

- 完整规则引擎状态；
- 未公开的手牌或牌山；
- 随机种子、认证信息和数据库 ID；
- 仅供反作弊或运维使用的标记。

## 错误信封

```json
{
  "kind": "error",
  "schema": "error.v1",
  "command_id": "cmd_opaque",
  "code": "game.stale_version",
  "message": "game state has advanced",
  "retryable": true,
  "details": {
    "current_version": 44
  }
}
```

初始稳定错误类别：

```text
auth.*          身份或权限
request.*       格式、字段、大小限制
room.*          房间状态和成员操作
matchmaking.*   队列状态
game.*          版本、回合、动作合法性
server.*        暂时故障
```

服务端日志记录内部错误链和 correlation ID，不能把堆栈返回客户端。

## 流控与兼容

- 服务端限制单消息大小、每连接命令速率和未确认命令数；
- 慢连接丢弃可重建的 presence 更新，但不丢牌局事件；
- 无法追上事件流时关闭连接，客户端以快照恢复；
- 心跳包含服务端时间和最新已发送 seq；
- 服务端声明最低客户端版本与支持的消息 schema；
- 增加可选字段属于兼容修改；删除、改义或改类型必须提升 payload schema。
