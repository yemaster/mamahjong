# 后端运行骨架

状态：可玩版已实现
最后更新：2026-07-31

## 启动顺序

1. 严格解析环境配置；
2. 初始化结构化日志；
3. 创建应用状态与路由；
4. 绑定监听地址；
5. 所有启动依赖可用后标记 ready；
6. 收到 SIGINT/SIGTERM 后先取消 ready，再排空连接。

未来数据库、规则注册表和桌局监督器必须在第 5 步前完成初始化。启动失败
直接退出，不以不完整功能继续提供服务。

## 环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `MAMAHJONG_BIND_ADDRESS` | `127.0.0.1:8080` | HTTP 监听地址 |
| `MAMAHJONG_DATA_DIR` | `data` | 单局与整场记录归档目录 |
| `MAMAHJONG_ADMIN_WEB_DIR` | `apps/admin-web/dist` | 管理端生产构建目录 |
| `MAMAHJONG_ADMIN_PASSWORD` | 空 | 空值关闭管理登录 |
| `RUST_LOG` | `info` | tracing 过滤规则；非法值导致启动失败 |

运行日志同时输出到标准输出和
`<MAMAHJONG_DATA_DIR>/logs/server.jsonl.YYYY-MM-DD`。审计日志独立写入
`<MAMAHJONG_DATA_DIR>/audit/audit.jsonl`，不受 `RUST_LOG` 影响。

默认只监听本机，公开部署必须显式配置地址并由 TLS 反向代理接入。

## 健康检查

- `GET /health/live`：进程事件循环仍可响应；
- `GET /health/ready`：进程可以接收新流量。

ready 在应用完成初始化后才变为 true，优雅停机开始时立即变为 false。
健康接口不访问数据库；未来依赖故障通过后台状态更新 readiness，避免每次
探测放大依赖故障。

## 代码边界

- `main.rs`：进程组装、监听与信号；
- `config.rs`：严格配置解析；
- `lib.rs`：应用状态和顶层路由；
- `health.rs`：健康状态与 HTTP 投影。

业务路由将按上下文挂载到 `/api/v1`，不会继续堆入 `main.rs`。
