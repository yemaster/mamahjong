# 通信 API 设计

状态：M0 设计基线  
最后更新：2026-07-29

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
- 写请求通过 `Idempotency-Key` 或消息 `command_id` 幂等。

## HTTP 资源草案

```text
POST   /api/v1/sessions/guest
POST   /api/v1/ws-tickets

GET    /api/v1/rule-sets
GET    /api/v1/rule-sets/{rule_set_id}
POST   /api/v1/rule-sets/{rule_set_id}/validate

POST   /api/v1/rooms
GET    /api/v1/rooms/{room_id}
PATCH  /api/v1/rooms/{room_id}
POST   /api/v1/rooms/{room_id}/members
DELETE /api/v1/rooms/{room_id}/members/me
PUT    /api/v1/rooms/{room_id}/members/me/readiness
POST   /api/v1/rooms/{room_id}/matches

POST   /api/v1/matchmaking-tickets
GET    /api/v1/matchmaking-tickets/{ticket_id}
DELETE /api/v1/matchmaking-tickets/{ticket_id}

GET    /api/v1/matches/{match_id}
GET    /api/v1/matches/{match_id}/result
GET    /api/v1/matches/{match_id}/hands
GET    /api/v1/matches/{match_id}/hands/{hand_id}
GET    /api/v1/matches/{match_id}/hands/{hand_id}/replay
GET    /api/v1/matches/{match_id}/replay
```

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

整场记录返回当时的完整规则快照和按序单局摘要。单局复盘接口只读取该局
起止事件范围；整场复盘按 `hand_index` 串联所有单局。未结束牌局的隐藏信息
按请求者权限裁剪。

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
