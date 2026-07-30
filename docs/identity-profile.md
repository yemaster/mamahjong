# 用户注册与档案

状态：M5 设计基线
最后更新：2026-07-30

## 模型

账号身份与游戏展示档案分离：

```text
User
├── id, version
├── login_name_canonical
├── status
├── credential
└── profile
    ├── nickname
    ├── equipped_title_id?
    ├── selected_character_id?
    └── ranks[]
```

- `UserId` 永久且不可猜测，昵称、登录名和角色都不能作为外键；
- 登录名规范化后唯一，昵称允许重复并可修改；
- 密码只保存带算法和参数的强哈希，日志、事件和响应不得出现密码或哈希；
- 称号、角色、段位是独立目录/关联，不用 JSON 杂项字段承载；
- 房间和整场记录保存开局时的昵称、称号、角色和段位展示快照，之后改名不
  修改历史记录。

昵称首版按 Unicode 字符数限制为 2 到 24，去除首尾空白并拒绝控制字符。
内容审核是应用服务，不改变领域值对象的确定性校验。

## 注册

首版注册输入：

```json
{
  "login_name": "player_01",
  "password": "client secret",
  "nickname": "雀士一号"
}
```

成功时在一个事务内创建用户、凭据、档案和初始会话。重复登录名返回稳定
冲突错误；响应只包含用户、档案和会话，不回显密码。

## 扩展关联

- `title_catalog` 定义版本化称号；`user_titles` 保存取得事实；
- `character_catalog` 定义角色；`user_characters` 保存拥有状态；
- `user_ranks` 以 `rule_set_id + queue_id` 分区保存段位、积分和版本；
- 档案中的装备引用必须指向用户已经拥有且仍可用的目录项；
- 删除目录项时保留历史展示快照，不能破坏战绩回放。

API 外壳从首版就返回稳定的 `equipped_title`、`selected_character` 和
`ranks` 容器；尚未获得时分别为 `null` 或空数组，后续增加内容不修改用户
身份格式。
