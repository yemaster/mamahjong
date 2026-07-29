# Mamahjong

可扩展的在线麻将服务。当前处于基础框架阶段。

## 工程结构

```text
apps/server/          后端进程入口
crates/mahjong-core/  不依赖网络和存储的领域核心
docs/                 架构与迭代计划
```

## 本地检查

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

设计约束见 [架构文档](docs/architecture.md)，开发顺序见
[路线图](docs/roadmap.md)。

