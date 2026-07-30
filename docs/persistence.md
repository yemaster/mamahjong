# 对局存储设计

状态：M0 设计基线  
最后更新：2026-07-29

## 设计结论

对局采用“追加事件 + 周期快照”，业务数据使用关系表。事件用于审计、恢复
和回放，快照用于快速加载。Redis 只保存可丢失的在线状态与匹配队列，不是
牌局事实来源。

选择理由记录于 [ADR-0002](adr/0002-game-persistence.md)。

## 数据归属

| 数据 | 主存储 | 说明 |
|---|---|---|
| 用户、房间、成员 | PostgreSQL 关系表 | 需要约束与查询 |
| 规则快照 | PostgreSQL JSONB | 不可变、带版本 |
| 对局事件 | PostgreSQL 追加表 | 每桌严格有序 |
| 对局快照 | PostgreSQL 二进制列 | 压缩并加密 |
| 每局与整场结果 | PostgreSQL 关系表 | 永久索引、可审计 |
| 段位变更 | PostgreSQL 关系表 | 与整场结果关联 |
| 在线连接、短期票据 | Redis | 可重建 |
| 匹配队列 | Redis | 带过期时间 |

## 逻辑表

### users 与 user_profiles

```text
users:
id, version, login_name_canonical, status, created_at, updated_at

user_credentials:
user_id, password_hash, algorithm, parameters_json, updated_at

user_profiles:
user_id, version, nickname, nickname_normalized,
equipped_title_id, selected_character_id, updated_at
```

登录名规范化后唯一，昵称允许重复。称号、角色、段位分别使用
`title_catalog/user_titles`、`character_catalog/user_characters` 和
`user_ranks`；档案只保存当前装备引用。比赛玩家表保存开局展示快照，不能
通过联表读取“当前昵称”替代历史昵称。

### rooms

```text
id, version, owner_user_id, name, visibility, lifecycle,
rule_snapshot_json, active_match_id, created_at, updated_at
```

`room_members` 单独保存座位、准备、加入顺序和连接状态。数据库唯一约束
保证同一房间的座位不重复、用户不重复。

### matches

```text
id, version, rule_set_id, engine_version, lifecycle,
rule_snapshot_json, last_event_seq, snapshot_seq,
created_at, finished_at
```

`match_players` 保存固定座次和开局身份快照。昵称等展示信息使用开局快照，
避免用户改名破坏历史记录。

### hand_records

```text
id, match_id, hand_index, engine_hand_label,
started_event_seq, ended_event_seq, dealer_seat,
result_type, result_json, score_delta_json,
started_at, ended_at
```

每一局都必须有一条记录，包含和牌、荒牌流局、途中流局等结束类型。主键
`id` 全局唯一，`(match_id, hand_index)` 唯一。`engine_hand_label` 仅供展示，
推进逻辑仍由规则引擎决定。

局结束事务同时写入：

- 最后一批领域事件；
- `hand_records` 结果；
- 该事件序号的强制快照；
- 对应 outbox 消息。

因此不能出现牌局已经推进但单局战绩缺失的状态。

### match_results

```text
match_id, format, final_scores_json, placements_json,
uma_oka_json, result_json, finished_event_seq, created_at
```

东风战、半庄或其他赛制结束时都保存一条整场结果。完整 `RuleSnapshot`、
固定玩家信息和起止时间从 `matches`、`match_players` 联合读取。排名和马点
保存计算后的结果，同时保留原始点数，避免以后规则展示变化。

### game_events

```text
match_id, seq, event_name, event_version, actor_user_id,
public_payload_json, secret_payload_ciphertext,
encryption_key_id, occurred_at, causation_id, correlation_id
```

主键为 `(match_id, seq)`。`seq` 在单桌内连续递增。字段约束：

- `event_name` 使用命名空间，如 `riichi.tile_discarded`；
- `event_version` 只描述该事件 payload，不等于 API 版本；
- `causation_id` 对应客户端命令 ID，用于幂等；
- `correlation_id` 串联一次用例产生的多个事件；
- 隐藏牌、完整牌山、随机种子等只进入加密 secret payload；
- public payload 仍需经过网络投影，不保证可以原样广播。

### game_snapshots

```text
match_id, seq, snapshot_schema, codec, state_ciphertext,
encryption_key_id, state_hash, created_at
```

初期 `codec` 使用 `json+zstd`。不采用 Rust 内存布局或 `bincode` 作为长期
格式，避免编译器和结构调整导致历史数据不可读。

### command_receipts

```text
scope_id, actor_user_id, command_id, request_hash,
first_event_seq, last_event_seq, result_json, expires_at
```

重复命令返回原结果；同一 `command_id` 携带不同内容时拒绝。

### outbox

牌局事件、结果和待广播通知在同一数据库事务内写入 outbox，后台发布成功
后标记完成，防止“数据已提交但消息丢失”。

## 原子写入

处理一个桌局命令时，单个事务执行：

1. 校验当前 `matches.version` 和 `last_event_seq`；
2. 校验或读取 `command_receipts`；
3. 追加一个或多个事件；
4. 更新 match 版本，按策略写快照；
5. 写入 receipt 与 outbox；
6. 提交后再向连接广播。

进程内采用每桌单写任务减少锁竞争；数据库的版本条件和唯一键仍是最终
保护，避免故障转移时双写。

## 快照策略

- 每个小局结束时强制快照；
- 进行中的小局每固定事件数快照，阈值由运行配置决定；
- 载入时读取最新快照，再顺序应用后续事件；
- 快照校验失败时退回上一个快照；
- 状态 hash 用于检测损坏，不用于替代加密认证。

## 留存与查询

- `game_events`、`hand_records`、`match_results` 和规则快照默认不自动过期；
- 热库容量达到阈值后，可以把已结束整场的事件与快照迁移到不可变冷存储；
- 冷存储迁移必须保留校验 hash 和可查询索引，API 行为不能变成“仅剩总分”；
- 每局记录可独立查询，也可定位到该局事件范围进行复盘；
- 整场记录列出所有单局，并支持按原始顺序完整复盘；
- 测试牌局与正式牌局通过数据分类字段区分，避免污染段位统计，但同样留存。

## 随机性与复盘

- 生产环境随机源必须为 CSPRNG；
- 每局保存加密种子或完整确定性输入，不能依赖进程全局 RNG；
- 开局可以发布种子承诺 hash，结束后按房间策略公开验证材料；
- 未来可增加客户端共同贡献随机种子，不改变事件外壳；
- 普通复盘视图在牌局结束前不能读取 secret payload。

## 迁移与保留

- 外壳、事件、快照各自独立版本；
- 读取器至少支持当前版本和仍在保留期内的历史版本；
- 破坏性变更采用“新增读取器 → 后台迁移 → 切换写版本 → 下线旧读取器”；
- 原始事件只追加不原地改写，迁移结果写入新快照；
- 账号删除与战绩保留分离，历史玩家可匿名化。
