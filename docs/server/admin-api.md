# 管理后台 API

基础路径：`/api/v1/admin`。除会话初始化和登录外均要求管理员会话；写操作还要求 `x-csrf-token`。

## 会话与总览

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET、POST、DELETE | `/session` | 初始化、登录、退出 |
| GET | `/me` | 当前管理员 |
| GET | `/overview` | 用户、房间、对局、素材和审计统计 |

## 对局与运营

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET | `/matches` | 已归档对局列表 |
| GET | `/matches/:id` | 对局完整记录 |
| GET | `/rooms` | 房间列表 |
| POST | `/rooms/:id/close` | 关闭等待中的房间 |
| GET | `/users` | 用户列表 |
| PUT | `/users/:id` | 编辑用户昵称 |
| PUT | `/users/:id/status` | 更新账号状态 |

## 素材

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET、POST | `/characters` | 列表、添加 |
| PUT、DELETE | `/characters/:id` | 编辑、删除 |
| GET、POST | `/tablecloths` | 列表、添加 |
| PUT、DELETE | `/tablecloths/:id` | 编辑、删除 |
| GET、POST | `/music` | 列表、添加 |
| PUT、DELETE | `/music/:id` | 编辑、删除 |
| GET、DELETE | `/assets?path=...` | 浏览或递归删除持久化资源 |
| POST | `/assets/folders` | 在指定路径新建文件夹 |
| POST | `/assets/files?path=...&name=...` | 上传二进制文件，最大 50 MB |

素材导入由前端读取版本化 JSON，并按编号调用上述新增或编辑接口；导出不请求服务端。文件格式见[素材导入导出](../admin/import-export.md)。
运行时文件通过 `/user-assets/...` 公开读取；路径会执行目录穿越和符号链接校验，
写操作同时受管理员会话与 CSRF 保护。

## 系统

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET | `/database` | 持久化状态、业务表及记录数 |
| GET | `/audit` | 最近 500 条审计记录 |

列表响应使用版本化 `schema`。当前数据量由服务端一次返回，前端完成筛选和分页；后续数据量增大时可增加 `page`、`page_size`、`keyword` 参数。
