# 麻麻的将 — 项目文档

## 架构

| 文档 | 说明 |
|---|---|
| [架构设计总览](architecture/overview.md) | 系统分层、依赖规则、核心扩展点与质量门槛 |
| [对象模型](architecture/domain-model.md) | 限界上下文、聚合、不变量、标识符与生命周期 |
| [后端运行骨架](architecture/server-runtime.md) | 启动顺序、环境变量、健康检查与代码边界 |

## 游戏引擎

| 文档 | 说明 |
|---|---|
| [日麻基础模型](engine/riichi-model.md) | 牌对象、牌集合、洗牌与牌山分区 |
| [日麻规则配置](engine/riichi-rules.md) | 配置结构、校验、预设与版本化快照 |
| [日麻单局状态机](engine/riichi-hand-state.md) | 聚合结构、命令、响应窗口、立直与流局 |
| [日麻和牌与计分](engine/riichi-scoring.md) | 牌形搜索、役种目录、符计算与结算 |
| [冲击麻将规则](engine/impact-rules.md) | 第二套规则集：自摸限定、财神、杠点与全交 |
| [对局推进与前后端同步](engine/match-progression.md) | 局间推进、终局条件、动画同步契约 |

## 通信协议

| 文档 | 说明 |
|---|---|
| [通信 API 设计](protocol/api.md) | HTTP 资源、WebSocket 建连、命令/事件/错误信封 |
| [实时传输](protocol/realtime-transport.md) | WS 实现、事件裁剪、游标续传与视图增量同步 |
| [操作时限与断线](protocol/turn-timer.md) | 两段式时钟、超时动作、到期扫描与重连 |

## 服务端

| 文档 | 说明 |
|---|---|
| [对局存储设计](server/persistence.md) | 事件溯源、逻辑表设计、快照策略与当前实现 |
| [用户注册与档案](server/identity-profile.md) | 身份与档案分离、注册流程与扩展关联 |
| [管理台与审计](server/admin-console.md) | 管理功能、审计日志、HTTP 接口与页面约束 |
| [牌谱与重演](server/match-record-replay.md) | 记录生成、归档、接口与客户端重演 |

## 客户端

| 文档 | 说明 |
|---|---|
| [网页客户端设计](client/game-web.md) | React + PixiJS 架构、场景图、WS 集成与桌面分发 |
| [对局牌桌视觉规范](client/visual-spec.md) | 几何约束、动画、手牌、副露、结算演出与牌谱界面 |
| [素材管线](client/asset-pipeline.md) | 纹理图集、角色素材、音效与降级策略 |
| [牌效机器人](client/bot.md) | 独立 HTTP 客户端、向听数策略与受入计算 |

## 部署

| 文档 | 说明 |
|---|---|
| [部署与运行](deployment/overview.md) | Docker Compose 部署、配置、健康检查与排障 |

## 项目

| 文档 | 说明 |
|---|---|
| [开发路线](project/roadmap.md) | M0–M9 里程碑、完成状态与实施纪律 |
| [开发规范](project/develop-standard.md) | 代码修改流程、文档要求与提交约定 |

## 架构决策记录

| 编号 | 决策 |
|---|---|
| [ADR-0001](adr/0001-modular-monolith.md) | 模块化单体优先 |
| [ADR-0002](adr/0002-game-persistence.md) | 追加事件与周期快照 |
| [ADR-0003](adr/0003-rule-engine-boundary.md) | 规则引擎拥有玩法状态 |
| [ADR-0004](adr/0004-container-deployment.md) | OCI 容器作为标准部署单元 |
| [ADR-0005](adr/0005-versioned-rule-config.md) | 完整且版本化的规则快照 |
| [ADR-0006](adr/0006-authoritative-hand-state-machine.md) | 单写权威状态机与判定器边界 |
| [ADR-0007](adr/0007-pure-scoring-and-settlement.md) | 纯计分器与结算分层 |
| [ADR-0008](adr/0008-protocol-separated-clients.md) | 客户端以协议与服务端分离 |
| [ADR-0009](adr/0009-mvp-record-archive.md) | 可玩版原子 JSON 战绩归档 |
| [ADR-0010](adr/0010-call-rules-and-snapshot-schema-2.md) | 副露规则组与规则快照 schema 2 |
| [ADR-0011](adr/0011-second-rule-family-impact.md) | 接入第二套规则集（冲击麻将） |
