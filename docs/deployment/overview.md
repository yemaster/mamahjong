# 部署与运行

状态：可玩版已验证
最后更新：2026-07-31

## 环境要求

- Docker Engine
- Docker Compose v2 或更高版本

仅部署服务端时不需要在宿主机安装 Rust。

## 一键启动

在项目根目录执行：

```bash
docker compose up --detach --build
```

默认监听 `127.0.0.1:8080`。验证：

```bash
docker compose ps
curl --fail http://127.0.0.1:8080/health/ready
```

健康响应：

```json
{"status":"ok","service":"mamahjong-server","version":"0.2.0"}
```

## 配置

如需修改端口或日志等级：

```bash
cp .env.example .env
```

编辑 `.env`：

| 变量 | 默认值 | 用途 |
|---|---|---|
| `MAMAHJONG_WEB_HOST` | `127.0.0.1` | Web 出口的宿主机监听地址 |
| `MAMAHJONG_WEB_PORT` | `8080` | Web 出口的宿主机端口 |
| `MAMAHJONG_ADMIN_WEB_URL` | `http://admin-web:8080` | Web 容器转发 `/admin` 的内部地址 |
| `MAMAHJONG_ADMIN_SERVER_URL` | `http://server:8080` | 管理端代理的服务端地址 |
| `MAMAHJONG_DATABASE_URL` | Compose 数据库 | 完整 PostgreSQL 地址 |
| `RUST_LOG` | `info` | 日志过滤规则 |
| `MAMAHJONG_ADMIN_LOGIN_NAME` | `admin` | 管理员账号 |
| `MAMAHJONG_ADMIN_PASSWORD` | 空 | 管理员密码；空值关闭管理登录 |
| `MAMAHJONG_ADMIN_NICKNAME` | `管理员` | 管理员昵称 |
| `MAMAHJONG_ADMIN_COOKIE_SECURE` | `false` | HTTPS 部署时设为 `true` |

容器内部固定监听 `0.0.0.0:8080`。`.env` 不会进入镜像，也不应提交到 Git。
单局和整场记录保存在 Compose 命名卷 `mamahjong_match-records`，管理端上传的运行时
静态资源保存在 `mamahjong_user-assets`。执行普通 `docker compose down` 不会删除这些卷；
只有显式使用 `docker compose down --volumes` 才会一并删除持久化数据。

启用管理端：

```bash
cp .env.example .env
```

设置 `MAMAHJONG_ADMIN_PASSWORD` 后启动可选管理端：

```bash
docker compose --profile admin up --detach
```

访问 `http://127.0.0.1:8080/admin/`。管理端容器不暴露宿主机端口；浏览器
通过主 Web 容器访问管理页面和同源 `/api/`，由
管理端镜像代理到 `MAMAHJONG_ADMIN_SERVER_URL`；数据库连接由服务端负责。
运行日志写入卷内 `logs/`，审计日志写入
`audit/audit.jsonl`，牌局记录写入 `matches/`。

如需让反向代理从其他主机访问，可以把 `MAMAHJONG_WEB_HOST` 改为内网地址。
不要在没有 TLS、鉴权和防火墙的情况下直接暴露公网。

## 常用命令

查看状态：

```bash
docker compose ps
```

查看日志：

```bash
docker compose logs --follow --tail=200 server
```

重启：

```bash
docker compose restart server
```

停止并移除容器：

```bash
docker compose down
```

更新代码后的重新部署：

```bash
docker compose build --pull server
docker compose up --detach server
```

Compose 会向旧进程发送 SIGTERM。服务先取消 readiness，再在最长 30 秒内
排空已有连接。

## 独立构建和运行

不使用 Compose 时：

```bash
docker build --tag mamahjong-server:local .
docker run --detach \
  --name mamahjong-server \
  --publish 127.0.0.1:8080:8080 \
  --read-only \
  --mount type=volume,source=mamahjong-match-records,target=/var/lib/mamahjong \
  --env MAMAHJONG_DATA_DIR=/var/lib/mamahjong \
  --tmpfs /tmp:size=16m \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  mamahjong-server:local
```

本地直接启动服务端时，归档默认写入项目根目录的 `data/`。可以修改：

```bash
MAMAHJONG_DATA_DIR=/path/to/records cargo run -p mamahjong-server
```

## 启动机器人测试

服务端启动后运行：

```bash
cargo run -p mamahjong-bot -- --all
```

机器人会为四麻、三麻各创建一个东风测试房并打至整场结束。连接其他地址：

```bash
cargo run -p mamahjong-bot -- \
  --all --server http://127.0.0.1:18080
```

机器人是独立 HTTP 客户端，不需要与服务端部署在同一台机器。

## 生产部署基线

- 在容器前放置支持 WebSocket 的 TLS 反向代理；
- 只向反向代理开放容器端口；
- 使用镜像仓库中的不可变 tag 或 digest，不使用浮动 `latest`；
- 在常规依赖升级中显式更新 Dockerfile 的基础镜像 digest 并重新执行验收；
- 将日志采集到外部系统；
- 对 `/health/live` 使用存活探测，对 `/health/ready` 使用就绪探测；
- 升级时先等待新实例 ready，再停止旧实例；
- 数据库和 Redis 上线后使用外部服务或持久卷并配置备份。

当前账号、会话、房间和进行中的牌局仍在内存中；服务重启后不能恢复进行中
的牌局。已写入卷的 `match_record.v1` 单局与整场记录不会随容器重建丢失。
PostgreSQL 阶段会增加可索引的在线历史查询、运行中事件事务和牌局恢复。

运行镜像使用无 shell 的 Distroless 基础镜像和 UID/GID `65532:65532`。
容器健康检查由镜像内的 `mamahjong-healthcheck` 执行，不依赖 curl 或 shell。

## 源码开发管理端

先启动服务端，再运行：

```bash
cd apps/admin-web
npm install
npm run dev
```

Vite 将 `/api` 代理到 `127.0.0.1:8080`。提交前执行：

```bash
npm run typecheck
npm test
npm run build
```

## 排障

端口被占用：

```bash
MAMAHJONG_WEB_PORT=18080 docker compose up --detach
```

检查健康状态：

```bash
docker inspect \
  --format '{{json .State.Health}}' \
  "$(docker compose ps --quiet server)"
```

检查最终 Compose 配置：

```bash
docker compose config
```

若容器反复重启，先运行 `docker compose logs server`。无效的 `RUST_LOG`
配置或端口绑定失败会让进程直接退出。
