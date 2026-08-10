#!/bin/bash

# 停止所有机器人的脚本

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}正在停止所有机器人...${NC}"

# 从 PID 文件读取并终止进程
if [ -f ".bot_pids" ]; then
    while read -r pid; do
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${BLUE}终止进程 ${pid}...${NC}"
            kill "$pid" 2>/dev/null || kill -9 "$pid" 2>/dev/null
        fi
    done < .bot_pids
    rm -f .bot_pids
    echo -e "${GREEN}已停止所有机器人${NC}"
else
    echo -e "${YELLOW}没有找到运行中的机器人${NC}"
fi

# 额外保险：终止所有 mamahjong-bot 进程
pkill -f mamahjong-bot && echo -e "${GREEN}已清理所有 mamahjong-bot 进程${NC}" || echo -e "${YELLOW}没有其他机器人进程${NC}"
