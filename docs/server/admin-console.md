# 管理台与审计

状态：已实现
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

管理端是独立的 `apps/admin-web` 应用：

- React 19 + TypeScript + Vite；
- Ant Design Pro Components 提供后台布局、页容器、统计卡片、数据表格和登录页；
- 浏览器 History API 管理四个固定页面路由；
- TanStack Query 管理查询缓存、提交状态和失效刷新。

前端不包含自定义样式表。颜色、圆角、间距与字体通过 Ant Design 的主题令牌
和组件布局属性配置。布局采用 232px 固定侧栏、固定页头和流式内容区；页面
使用 24px 内容间距，窄屏由 `ProLayout` 自动切换移动端导航。筛选器放入
表格工具栏，避免额外卡片切断页面层级。页面不使用图表和装饰性说明。

开发时，Vite 将 `/api` 代理到服务端。生产构建使用 `/admin/` 作为基础路径，
静态文件由服务端提供，因此浏览器只访问同一来源，不引入额外的跨域配置。

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
写操作在 `X-CSRF-Token` 请求头中提交同步令牌；令牌不进入 URL、运行日志或
审计日志。

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

## HTTP 接口

```text
GET    /api/v1/admin/session
POST   /api/v1/admin/session
DELETE /api/v1/admin/session
GET    /api/v1/admin/me
GET    /api/v1/admin/overview
GET    /api/v1/admin/users
PUT    /api/v1/admin/users/{user_id}/status
GET    /api/v1/admin/rooms
POST   /api/v1/admin/rooms/{room_id}/close
GET    /api/v1/admin/audit

GET    /admin/
GET    /admin/*
```

管理接口统一返回 JSON。认证失败返回 `401`，权限不足返回 `403`，状态冲突
返回 `409`。写操作成功后由 TanStack Query 使关联列表和概览缓存失效。

`/admin/*` 对前端路由回退到 `index.html`。静态响应设置 CSP、禁止页面被
嵌入；管理接口禁止缓存。

## 页面约束

- 登录页使用无图片双栏布局，只保留品牌、账号、密码和登录按钮；
- 概览页首行显示账号数、等待房间、进行中房间，下面显示最近审计；
- 用户页按昵称或登录名筛选，状态操作放在表格末列并要求确认；
- 房间页按状态和模式筛选，仅等待中的房间显示关闭操作；
- 审计页按类别、结果和关键字筛选，默认时间倒序；
- 表格使用稳定 ID 作为行键，加载、空状态和错误状态使用 Ant Design 组件；
- 删除、关闭、停用等操作不得只依靠颜色表达含义。

## 验收

- 未配置管理员时管理台不可用；
- 玩家账号不能进入管理台；
- 登录、退出、过期、CSRF 错误均有测试；
- 用户停用会撤销会话，当前管理员不能停用自己；
- 进行中房间不能关闭，等待房间可以关闭；
- 审计文件重启可读，篡改后启动失败；
- 前端构建、类型检查和关键页面测试通过；
- 管理接口不会把密码、会话或 CSRF 令牌写入日志；
- 页面在桌面和窄屏下均可使用；
- Docker 中运行日志、审计日志和牌局记录都写入持久卷。
