#!/bin/bash

# 三机器人一键启动脚本
# 用法: ./run-bots.sh <房间号>

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检查房间号参数
if [ -z "$1" ]; then
    echo -e "${RED}错误: 请提供房间号${NC}"
    echo "用法: $0 <房间号>"
    exit 1
fi

ROOM_ID="$1"

# 机器人账号信息
BOT1_USER="abc1"
BOT1_PASS="abc1abc1abc1"

BOT2_USER="abc2"
BOT2_PASS="abc2abc2abc2"

BOT3_USER="abc3"
BOT3_PASS="abc3abc3abc3"

# 服务器地址（可以通过环境变量覆盖）
SERVER_URL="${MAMAHJONG_SERVER_URL:-http://127.0.0.1:8080}"

echo -e "${YELLOW}════════════════════════════════════════${NC}"
echo -e "${YELLOW}  🀫  三机器人一键启动  🀫${NC}"
echo -e "${YELLOW}════════════════════════════════════════${NC}"
echo ""
echo -e "${BLUE}● 服务器: ${SERVER_URL}${NC}"
echo -e "${BLUE}● 房间号: ${ROOM_ID}${NC}"
echo -e "${BLUE}● 机器人: ${BOT1_USER}, ${BOT2_USER}, ${BOT3_USER}${NC}"
echo ""

# 检查是否已构建机器人
if [ ! -f "target/release/mamahjong-bot" ] && [ ! -f "target/debug/mamahjong-bot" ]; then
    echo -e "${YELLOW}正在构建机器人...${NC}"
    cargo build --release -p mamahjong-bot
    echo ""
fi

# 确定使用哪个二进制文件
if [ -f "target/release/mamahjong-bot" ]; then
    BOT_BIN="target/release/mamahjong-bot"
else
    BOT_BIN="target/debug/mamahjong-bot"
fi

echo -e "${GREEN}使用二进制: ${BOT_BIN}${NC}"
echo ""

# 创建日志目录
mkdir -p logs

# 启动单个机器人的函数
start_bot() {
    local bot_num=$1
    local username=$2
    local password=$3
    local log_file="logs/bot${bot_num}_$(date +%Y%m%d_%H%M%S).log"

    echo -e "${BLUE}[Bot ${bot_num}] 正在启动 ${username}...${NC}"

    # 使用命令行参数自动登录和加入房间
    MAMAHJONG_SERVER_URL="$SERVER_URL" "$BOT_BIN" \
        --username "$username" \
        --password "$password" \
        --room "$ROOM_ID" \
        --quiet 2>&1 | while IFS= read -r line; do
        echo "[Bot ${bot_num}] $line"
    done | tee "$log_file" &

    local pid=$!
    echo -e "${GREEN}[Bot ${bot_num}] 已启动 (PID: ${pid})${NC}"
    echo "$pid" >> .bot_pids
}

# 清理旧的 PID 文件
rm -f .bot_pids

# 启动三个机器人
echo -e "${YELLOW}开始启动机器人...${NC}"
echo ""

start_bot 1 "$BOT1_USER" "$BOT1_PASS"
sleep 1
start_bot 2 "$BOT2_USER" "$BOT2_PASS"
sleep 1
start_bot 3 "$BOT3_USER" "$BOT3_PASS"

echo ""
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo -e "${GREEN}三个机器人已全部启动！${NC}"
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo ""
echo -e "${YELLOW}提示:${NC}"
echo -e "  - 日志保存在 logs/ 目录"
echo -e "  - 使用 Ctrl+C 停止所有机器人"
echo -e "  - 或运行: ${BLUE}./stop-bots.sh${NC}"
echo ""

# 等待所有后台进程
wait
