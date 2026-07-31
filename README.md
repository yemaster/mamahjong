# Mamahjong

可扩展的在线麻将服务。当前版本可通过终端客户端完成四麻或三麻对局。

## 工程结构

```text
apps/server/          后端进程入口
clients/console/      独立 HTTP 终端客户端
clients/bot/          独立牌效机器人与对局测试器
crates/mamahjong-application/ 用例、身份、房间和桌局编排
crates/mahjong-core/  不依赖网络和存储的领域核心
crates/mahjong-riichi/ 四麻/三麻日麻规则实现
docs/                 架构与迭代计划
```

## 开始游戏

```bash
docker compose up --detach --build
cargo run -p mamahjong-console
```

三麻需要打开三个终端客户端，四麻需要四个。每个客户端注册不同账号；一人
建房，其余玩家加入，所有人按 `Space` 准备，房主按 `s` 开始。建房界面按
`F3` 切换四麻/三麻。

常用按键：

- 登录/注册：`F2`、`Tab`、`Enter`
- 大厅：`n` 建房、方向键选房、`Enter` 加入
- 房间：`Space` 准备、`s` 开始
- 对局：方向键选牌，`d` 打牌，`r` 立直，`t` 自摸，`p` 过，`h` 荣和
- 副露：`Space` 标记手牌，`c` 吃，`o` 碰，`k` 杠，`a` 加杠

弃牌后无人可响应时会直接进入下一家摸牌。只有界面显示“可操作”时才需要
选择荣和、碰、杠、吃或按 `p` 过。

服务端地址不是默认值时：

```bash
cargo run -p mamahjong-console -- --server http://服务器地址:端口
```

## 机器人测试

牌效机器人会自动注册测试账号、建房并打完整场。默认依次测试四麻和三麻：

```bash
cargo run -p mamahjong-bot -- --all
```

只测试一种玩法：

```bash
cargo run -p mamahjong-bot -- --variant yonma --quiet
cargo run -p mamahjong-bot -- --variant sanma --quiet
```

机器人与服务端、终端客户端是三个独立程序，只通过 HTTP JSON 通信。策略与
限制见 [牌效机器人](docs/bot.md)。

## 本地启动与检查

```bash
cargo run -p mamahjong-server
cargo run -p mamahjong-console

cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

设计约束见 [架构文档](docs/architecture.md)，开发顺序见
[路线图](docs/roadmap.md)，完整容器操作见
[部署与运行](docs/deployment.md)，日麻配置格式见
[日麻规则配置](docs/riichi-rules.md)，单局推进与恢复约束见
[日麻单局状态机](docs/riichi-hand-state.md)，计分与逐局留存边界见
[日麻和牌与计分](docs/riichi-scoring.md)，终端及后续多端规划见
[客户端与 UI 演进](docs/client-ui.md)，自动对局见
[牌效机器人](docs/bot.md)。
