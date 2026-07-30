# Mamahjong

可扩展的在线麻将服务。基础框架、日麻牌山、规则配置和权威单局状态机已完成。

## 工程结构

```text
apps/server/          后端进程入口
crates/mahjong-core/  不依赖网络和存储的领域核心
crates/mahjong-riichi/ 四麻/三麻日麻规则实现
docs/                 架构与迭代计划
```

## 本地检查

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 启动后端

Docker 一键启动：

```bash
docker compose up --detach --build
curl http://127.0.0.1:8080/health/ready
```

本地 Rust 工具链启动：

```bash
cargo run -p mamahjong-server
curl http://127.0.0.1:8080/health/ready
```

设计约束见 [架构文档](docs/architecture.md)，开发顺序见
[路线图](docs/roadmap.md)，完整容器操作见
[部署与运行](docs/deployment.md)，日麻配置格式见
[日麻规则配置](docs/riichi-rules.md)，单局推进与恢复约束见
[日麻单局状态机](docs/riichi-hand-state.md)。
