# 管理台与审计

状态：设计确认，等待实现
最后更新：2026-07-31

## 范围

首版管理台处理三类工作：

- 用户：查看全部账号，停用或恢复账号；
- 房间：查看公开、私有、进行中和已关闭房间，关闭等待中的房间；
- 审计：按时间查看认证、管理和关键对局事件。

牌谱、规则编辑、段位调整和服务器配置暂不放进首版。管理台不直接修改内存
对象，所有写操作经过应用层用例和状态校验。

## 页面结构

管理台使用左侧分级导航、右侧单一内容区。

```text
麻麻的将
├── 运营
│   ├── 概览
│   ├── 用户
│   └── 房间
└── 系统
    └── 审计日志
```

概览只显示用户、等待房间、进行中房间和最近审计。列表页顶部只保留标题、
数量和必要筛选；主要内容使用表格，写操作放在每行末尾。

服务端使用 Askama 编译期模板。界面只使用 Bootstrap 组件和工具类，不包含
自定义 CSS、行内样式、图表或装饰性文案。Bootstrap 文件随服务端提供，不
依赖外部 CDN。

## 管理身份

普通注册只能创建玩家账号。服务启动时可通过环境变量创建一个管理员账号：

```text
MAMAHJONG_ADMIN_LOGIN_NAME
MAMAHJONG_ADMIN_PASSWORD
MAMAHJONG_ADMIN_NICKNAME
MAMAHJONG_ADMIN_COOKIE_SECURE
```

未配置管理员密码时，管理台保持关闭。外网部署必须启用 HTTPS，并把
`MAMAHJONG_ADMIN_COOKIE_SECURE` 设为 `true`。

管理登录成功后生成独立的随机会话和 CSRF 令牌。会话只保存在服务端内存，
浏览器 Cookie 使用 `HttpOnly + SameSite=Strict`，有效期八小时。所有管理
写操作提交同步令牌；令牌不进入 URL、运行日志或审计日志。

管理员不能停用当前登录账号。停用其他账号时立即撤销该账号的游戏会话。
关闭房间只允许作用于等待中的房间；进行中的房间不能强制关闭。

## 运行日志

运行日志使用 `tracing` 的五级级别：

| 级别 | 用途 |
|---|---|
| `ERROR` | 数据损坏、持久化失败、服务不可继续完成请求 |
| `WARN` | 鉴权失败、冲突、非法状态转换、可疑请求 |
| `INFO` | 启停、建房、开局、终局和管理写操作 |
| `DEBUG` | HTTP 方法、路径、状态码、耗时和状态推进 |
| `TRACE` | 仅本地诊断，不记录牌面或凭据 |

同一份结构化 JSON 同时写入标准输出和每日滚动文件
`<data-dir>/logs/server.jsonl.YYYY-MM-DD`。`RUST_LOG` 控制级别，但不得关闭
独立审计日志。

HTTP 日志不记录 Authorization、Cookie、请求体、响应体、密码、会话令牌或
CSRF 令牌。成功请求使用 `DEBUG`，4xx 使用 `WARN`，5xx 使用 `ERROR`。

## 审计日志

审计日志与运行日志分开，追加写入
`<data-dir>/audit/audit.jsonl`。每条记录为 `audit_event.v1`：

```json
{
  "schema": "audit_event.v1",
  "sequence": 42,
  "occurred_at": "2026-07-31T12:00:00.000Z",
  "severity": "info",
  "category": "admin",
  "action": "admin.user.suspended",
  "actor_id": "user_...",
  "target_type": "user",
  "target_id": "user_...",
  "outcome": "success",
  "detail": "账号已停用",
  "previous_hash": "...",
  "entry_hash": "..."
}
```

记录覆盖：

- 注册、登录成功与失败；
- 建房、加入、离开、开局、终局；
- 进入和取消匹配；
- 管理登录、退出、用户状态变更、房间关闭；
- 管理员查看审计日志。

审计内容只保存稳定 ID 和结果摘要，不保存密码、令牌、完整请求体、手牌或
聊天内容。字段经过 JSON 编码，换行不能注入新记录。

每条记录的 SHA-256 包含上一条哈希，形成连续链。启动时校验全部已有记录；
序号、哈希或 JSON 损坏会阻止服务进入 ready。文件写入后执行同步，管理写
操作在审计意图成功落盘后才执行。

首版在内存保留最近 2,000 条供管理台查询，文件保留完整记录。生产环境应把
卷备份到只读或集中日志系统；删除和保留周期由部署方策略控制。

## HTTP 路由

```text
GET  /admin/login
POST /admin/login
POST /admin/logout

GET  /admin
GET  /admin/users
POST /admin/users/{user_id}/status
GET  /admin/rooms
POST /admin/rooms/{room_id}/close
GET  /admin/audit

GET  /admin/assets/bootstrap.min.css
```

管理 POST 成功后返回 `303 See Other`，避免刷新页面重复提交。模板自动转义
用户内容；响应设置 CSP、禁止缓存、禁止页面被嵌入。

## 验收

- 未配置管理员时管理台不可用；
- 玩家账号不能进入管理台；
- 登录、退出、过期、CSRF 错误均有测试；
- 用户停用会撤销会话，当前管理员不能停用自己；
- 进行中房间不能关闭，等待房间可以关闭；
- 审计文件重启可读，篡改后启动失败；
- 模板渲染中用户输入被转义；
- 页面在桌面和窄屏下均可使用；
- Docker 中运行日志、审计日志和牌局记录都写入持久卷。
