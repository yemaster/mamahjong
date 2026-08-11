# 机器人测试脚本使用说明

## 快速启动三个机器人

使用 `scripts/run-bots.sh` 脚本可以一次性启动三个测试机器人：

```bash
./scripts/run-bots.sh <房间号>
```

### 示例

```bash
# 启动三个机器人加入房间 ROOM123
./scripts/run-bots.sh ROOM123
```

### 机器人账号

脚本会自动使用以下三个账号登录：
- **abc1** / abc1abc1abc1
- **abc2** / abc2abc2abc2  
- **abc3** / abc3abc3abc3

### 停止机器人

```bash
# 停止所有运行中的机器人
./scripts/stop-bots.sh

# 或者直接按 Ctrl+C
```

### 日志文件

所有机器人的输出会保存在 `logs/` 目录下，文件名格式为：
- `bot1_YYYYMMDD_HHMMSS.log`
- `bot2_YYYYMMDD_HHMMSS.log`
- `bot3_YYYYMMDD_HHMMSS.log`

### 自定义服务器地址

如果需要连接其他服务器，可以设置环境变量：

```bash
MAMAHJONG_SERVER_URL=http://your-server:8080 ./scripts/run-bots.sh ROOM123
```

### 注意事项

1. 首次运行时，脚本会自动构建机器人（如果尚未构建）
2. 三个机器人会间隔 1 秒依次启动，避免同时连接
3. 所有机器人的输出都会显示在终端，并同时保存到日志文件
4. 确保服务器正在运行并且可以访问
