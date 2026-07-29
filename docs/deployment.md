# 部署与运行

状态：M0 已实现  
最后更新：2026-07-29

## 环境要求

- Docker Engine
- Docker Compose v2 或更高版本

不需要在宿主机安装 Rust。

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
{"status":"ok","service":"mamahjong-server","version":"0.1.0"}
```

## 配置

如需修改端口或日志等级：

```bash
cp .env.example .env
```

编辑 `.env`：

| 变量 | 默认值 | 用途 |
|---|---|---|
| `MAMAHJONG_HOST` | `127.0.0.1` | 宿主机监听地址 |
| `MAMAHJONG_PORT` | `8080` | 宿主机端口 |
| `RUST_LOG` | `info` | 日志过滤规则 |

容器内部固定监听 `0.0.0.0:8080`。`.env` 不会进入镜像，也不应提交到 Git。

如需让反向代理从其他主机访问，可以把 `MAMAHJONG_HOST` 改为内网地址。
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
  --tmpfs /tmp:size=16m \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  mamahjong-server:local
```

## 生产部署基线

- 在容器前放置支持 WebSocket 的 TLS 反向代理；
- 只向反向代理开放容器端口；
- 使用镜像仓库中的不可变 tag 或 digest，不使用浮动 `latest`；
- 在常规依赖升级中显式更新 Dockerfile 的基础镜像 digest 并重新执行验收；
- 将日志采集到外部系统；
- 对 `/health/live` 使用存活探测，对 `/health/ready` 使用就绪探测；
- 升级时先等待新实例 ready，再停止旧实例；
- 数据库和 Redis 上线后使用外部服务或持久卷并配置备份。

当前 M0 服务尚未接入数据库，没有需要挂载的业务数据目录。后续每局、整场
记录及事件将写入 PostgreSQL/归档存储，不能保存在容器临时文件系统。

运行镜像使用无 shell 的 Distroless 基础镜像和 UID/GID `65532:65532`。
容器健康检查由镜像内的 `mamahjong-healthcheck` 执行，不依赖 curl 或 shell。

## 排障

端口被占用：

```bash
MAMAHJONG_PORT=18080 docker compose up --detach
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
