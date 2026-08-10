# 二次元网页客户端设计

状态：网页端已实现，桌面端设计中
最后更新：2026-08-01

## 目标

用 React + TypeScript + Vite + PixiJS 构建日麻网页客户端，实现从登录到结算的
完整对局闭环。视觉风格参考雀魂、麻雀一番街：暗色主题、扁平铺色（禁用渐变）、
合理布局。

## 技术选型

| 层 | 技术 | 理由 |
|---|---|---|
| 框架 | React 19 | 与 admin-web 统一生态 |
| 语言 | TypeScript 7 strict | 类型安全 > 运行时报错 |
| 构建 | Vite 8 | HMR 快，proxy 配置简单 |
| 数据 | @tanstack/react-query 5 | 服务端缓存、轮询、重试 |
| 状态 | Zustand 5 | 轻量、可在 React 外订阅（PixiJS 桥接） |
| 渲染 | PixiJS 8 | GPU 加速牌桌渲染 |
| 样式 | CSS Modules + CSS 自定义属性 | 无 UI 库依赖，完全控制视觉 |
| 路由 | 手写 hash 路由 | 复用 admin-web 模式，零依赖 |

不引入 antd、react-router 或 react-pixi。牌桌渲染用命令式 PixiJS，交互覆盖层用
React DOM。

## 场景图

```text
        ┌─────────────────────────────────────┐
        │               App                    │
        │  ┌─────────────────────────────────┐│
        │  │           AuthGate               ││
        │  │  (检查身份，未登录弹 LoginModal)   ││
        │  │  ┌───────────────────────────┐  ││
        │  │  │        SceneRouter         │  ││
        │  │  │  ┌───────┬───────┬───────┐│  ││
        │  │  │  │Lobby  │Room   │Game   ││  ││
        │  │  │  │       │Match..│Result ││  ││
        │  │  │  │Profile│Create │       ││  ││
        │  │  │  └───────┴───────┴───────┘│  ││
        │  │  └───────────────────────────┘  ││
        │  └─────────────────────────────────┘│
        └─────────────────────────────────────┘
```

六场景：`Lobby`, `Room`, `CreateRoom`, `Matchmaking`, `Game`, `Result`, `Profile`。
`Lobby` 是默认着陆页，其他场景通过 `#hash` 路由进入。

### 场景转换

```text
Login ──→ Lobby ──→ CreateRoom ──→ Room ──→ Game ──→ Result ──→ Lobby
  ↑                    │              ↑                  │
  └────────────────────┘              │                  │
                                      │                  │
                        Matchmaking ──┘                  │
                        (直接进入 Game)                   │
                                                         │
                                          Profile ←──────┘
```

### 回房间只走一次转场

好友房投票退出后，牌桌把玩家送回 `Room`。这一步只允许发生一次转场：
`SceneTransition` 每次场景变更都要放一遍聚拢与揭开的动画，来回跳两趟人眼看到
的就是接连闪好几下。

会来回跳的是缓存。房间页的房态走 react-query，键是 `["room", roomId, userId]`，
上一次写进缓存的那份正是**开局那一刻**拉到的——`active_match_id` 指着刚刚结束
的那局。玩家回到房间页时缓存里的旧房态先一步渲染出来，房间页照着它又把人送回
牌桌，牌桌看见 `terminated_by_exit_vote` 再把人送回房间，直到后台重拉的新房态
落地才停下来。

两条规矩：

- 房间页只认**这次进来之后**真正拉回来的房态才决定进不进对局；缓存里那份只用
  来先把界面画出来，不用来跳转。
- 牌桌因对局结束离开时，顺手把那间房的房态缓存丢掉：对局都结束了，手上那份必
  然过期，留着只会让下一个页面照着旧的走。

同一件事在正常终局（`Game → Result → Room`）上也成立，规矩写在房间页而不是退出
投票这条路径上，任何进入房间页的方式都受它约束。

```text
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  HTTP API     │────→│  react-query      │────→│  React       │
│  /api/v1/*   │     │  (服务端缓存)      │     │  Components  │
└──────────────┘     └──────────────────┘     └──────────────┘
                              ↑                       ↑
                              │                       │
┌──────────────┐     ┌───────┴──────────┐     ┌──────┴───────┐
│  WebSocket   │────→│  useGameStore     │────→│  PixiJS      │
│  /api/v1/ws  │     │  (Zustand)        │     │  GameTable   │
└──────────────┘     └──────────────────┘     └──────────────┘
```

- **HTTP API**：认证、房间、匹配、对局快照。走 `react-query`，自动缓存/轮询/重试。
- **WebSocket**：实时事件、时钟、在线状态。对局命令优先走 WS，断开时回退 HTTP。
- **Zustand**：桥接 WS 事件到 PixiJS（不用 React 生命周期中转）。
- **本地倒计时**：`clock.v1` 到达后 `setInterval(100ms)` 递减 `remaining_ms`，
  下次 `clock.v1` 到达时校正。

选择 Zustand 而非 React Context：
- PixiJS 命令式代码可直接 `gameStore.getState()` 读取最新状态，无需 React 重渲染。
- `gameStore.subscribe()` 支持增量更新 sprites，避免每帧重建整个 Component tree。
- 更简洁的 API：`useGameStore(s => s.clocks)` 代替多层 Provider 嵌套。

## 组件树

```text
App
├── Layout
│   ├── TopBar (logo, breadcrumb, profile link, WS status dot)
│   └── Main
│       ├── LobbyScene
│       │   ├── RoomList
│       │   ├── QuickMatchButtons
│       │   └── CreateRoomModal
│       ├── RoomScene
│       │   ├── MemberGrid
│       │   ├── RuleSummary
│       │   └── ActionButtons (ready, start, leave)
│       ├── MatchmakingScene
│       │   ├── WaitingAnimation
│       │   └── CancelButton
│       ├── GameScene
│       │   ├── GameTable (PixiJS canvas)
│       │   ├── ClockBar (overlay)
│       │   ├── ActionPanel (overlay)
│       │   └── CharacterSide (overlay, placeholder)
│       ├── ResultScene
│       │   ├── PlacementBoard
│       │   └── HandHistory
│       └── ProfileScene
│           ├── CharacterSelect
│           └── StatsSummary
└── LoginModal (conditional overlay)
```

## 牌桌布局

### 四麻

```text
         ┌──────────────────────┐
         │    对家 (seat 2)      │
         │  牌河  手牌背  副露    │
         │                      │
 seat 3  │     中心(宝牌/场况)    │  seat 1
 左家   │  各家牌河交错排列      │  右家
         │                      │
         │    自家 (seat 0)      │
         │  副露  手牌面  摸牌    │
         └──────────────────────┘
```

### 三麻

```text
         ┌──────────────────────┐
         │    对家 (seat 1)      │
         │                      │
         │     中心(宝牌/场况)    │
         │                      │
         │    自家 (seat 0)      │
         └──────────────────────┘
              (seat 2 在右上)
```

`TableLayout` 类基于 `seatCount` 和画布尺寸计算所有元素坐标。渲染函数参数化
`seatCount`，四麻三麻共享同一套渲染逻辑。

## PixiJS 集成

### 生命周期

```typescript
// GameTable.tsx
useEffect(() => {  // mount — 仅 matchId 变化时重建
  const app = new Application();
  app.init({ resizeTo: container, background: 0x0d2818 });
  container.appendChild(app.canvas);
  appRef.current = app;
  return () => app.destroy(true);
}, [matchId]);

useEffect(() => {  // update — matchView 变化时增量修改
  updateSprites(appRef.current, layoutRef.current, matchView, mySeat);
}, [matchView.version]);
```

### 牌面渲染

`TileFactory` 类管理牌面纹理。优先加载 spritesheet PNG + JSON 图集，
失败时用后备占位渲染：

```typescript
// 占位牌面：PixiJS Graphics 绘制圆角矩形 + Text 显示 Unicode 牌名
function createPlaceholderTile(code: string): Container {
  const bg = new Graphics();
  bg.roundRect(0, 0, TILE_W, TILE_H, 4);
  bg.fill(getColor(code));  // 万=红, 筒=蓝, 索=绿, 字牌=深灰
  const text = new Text({ text: toUnicode(code), style: ... });
  // 赤牌加红边框
  return new Container().addChild(bg, text);
}
```

占位牌面始终可用，所以游戏在无任何素材时也能正常玩。

## 路由设计

复用 admin-web 的 `useSyncExternalStore` 模式。路径为 hash 编码：

```typescript
type GameScene =
  | { kind: "lobby" }
  | { kind: "room"; roomId: string }
  | { kind: "matchmaking"; ticketId: string }
  | { kind: "game"; matchId: string }
  | { kind: "result"; matchId: string }
  | { kind: "profile" };
```

URL 示例：
- `/#lobby` → LobbyScene
- `/#room/abc123` → RoomScene
- `/#game/xyz789` → GameScene

## CSS 主题

### 色彩系统

```css
:root {
  --color-bg:        #0D2818;  /* 深绿黑，全屏底色 */
  --color-felt:      #12523C;  /* 桌布 */
  --color-surface:   #1A1A2E;  /* 面板/卡片背景 */
  --color-text:      #F5EED6;  /* 象牙白，主文字 */
  --color-text-dim:  #888888;  /* 次要文字 */
  --color-accent:    #D4A853;  /* 黄金，按钮/选中/立直 */
  --color-danger:    #C44B4B;  /* 红，荣和/超时/报错 */
  --color-success:   #4ECCA3;  /* 绿，自摸/成功 */
  --color-offline:   #666666;  /* 离线/禁用 */
  --font: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
  --radius: 8px;
  --shadow: 0 2px 8px rgba(0,0,0,0.4);
}
```

规则：
- 禁用 CSS 渐变色（`linear-gradient`, `radial-gradient`）
- 层次用纯色 + `rgba(0,0,0,0.x)` 半透明叠加表示
- 按钮用纯色背景 + `:hover` 变亮 10%
- 边框用 `1px solid`，不用 `box-shadow` 模拟
- 字体统一 sans-serif，不混用 serif

### 响应式

桌面端优先，最小 1024×768。移动端不在本阶段范围。

```css
body { min-width: 1024px; min-height: 768px; overflow: hidden; }
```

## 对局交互

### 操作按钮

`ActionPanel` 根据 `phase`、`available_reactions`、`turn_actions` 动态显示：

| 阶段 | 可用按钮 |
|---|---|
| `AwaitingTurnAction` | 打牌(选中牌后)、立直(`riichi_discard_tile_ids`)、暗杠、加杠、自摸、流局(九种九牌，按钮上只写「流局」) |
| `AwaitingDiscard` | 打牌（仅选中摸到的牌或手中牌） |
| `AwaitingResponses` | 荣和、碰、吃、明杠、过 |
| `Ended` | (无操作, 1s 后跳转 ResultScene) |

响应阶段只有一种合法组合时直接提交（如碰牌只有一对），多种组合时才弹选择。

### 键盘快捷键

| 键 | 动作 |
|---|---|
| `← →` | 选牌 |
| `Space` | 切换牌标记 |
| `Enter` | 打牌 / 确认 |
| `R` | 立直打牌（须有可选牌） |
| `T` | 自摸 |
| `P` | 碰 / 过 |
| `C` | 吃 |
| `K` | 杠 |
| `H` | 荣和 |
| `9` | 九种九牌 |

### 倒计时条

`ClockBar` 浮动覆盖层：
- 每个计时座位旁显示剩余秒数（`remaining_ms / 1000`，向上取整）
- 颜色：绿色(>10s) → 黄色(5-10s) → 红色(<5s)
- 长考阶段（`base_ms == 0`）显示 "长考 Ns"

### 离线标记

每个玩家名字旁显示连接状态点：绿 = 在线，灰 = 离线。
来自 `presence.v1` 帧。

## WebSocket 集成

### 建连流程

```
1. POST /api/v1/ws-tickets → ticket
2. new WebSocket("ws://host/api/v1/ws?ticket={ticket}")
3. send { kind: "hello", protocol: "mamahjong.v1", subscriptions: [...] }
4. receive welcome
5. 事件循环：event/clock/presence/command_result/error/pong
```

### 断线处理

- 指数退避重连：500ms → 1s → 2s → ... → 60s
- 断开时标记 `wsState = "disconnected"`，界面显示 "离线，退回轮询"
- 回退 HTTP 轮询：`GET /api/v1/matches/{matchId}` 每 500ms
- 重连成功后：`hello` 带 `after_seq`，服务端补发缺失事件
- 重连后立即 HTTP 全量刷新一次确保状态一致

### 命令发送

优先 WebSocket，断开时回退 HTTP：
```
if (ws connected) → ws.send(command envelope)
else → POST /api/v1/matches/{id}/commands
```

命令后始终调用 HTTP `match_view` 刷新，而非等待 WS 事件（可能因竞态延迟）。

## 测试

- 单元：`api.ts` (fetch mock), `ws.ts` (Mock WebSocket), `TableLayout.ts` (坐标计算)
- 组件：各 Scene 渲染测试，mock API/WS
- 不测试 PixiJS 像素输出（测试坐标计算和 Container 结构即可）

## 暂不纳入

- 音效、语音、BGM（素材未提交，代码框架留好接口但功能不实现）
- 移动端适配
- 观战模式
- 牌谱回放
- 角色 Live2D 动画（用静态占位图）
- 道具/付费系统

## 桌面分发 (Tauri)

`apps/desktop` 用 Tauri v2 将 game-web 打包为独立桌面窗口。

### 架构

```text
┌──────────────────────────────────────┐
│           Tauri 原生窗口              │
│  ┌────────────────────────────────┐  │
│  │       WebView                   │  │
│  │  http://localhost:8080/game/    │  │
│  │  (或内嵌 game-web dist)         │  │
│  └────────────────────────────────┘  │
│  ┌────────────────────────────────┐  │
│  │   本地资源加载 (tauri://)        │  │
│  │   assets/characters/            │  │
│  │   assets/voices/                │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

桌面端连接本地或远程服务端。素材从本地文件系统加载，不再走 HTTP 的
`public/assets/` 路径。Tauri 提供 `tauri://localhost` 自定义协议或
文件系统 API 读取本地资源目录。

### 窗口配置

- 标题：麻麻的将
- 默认尺寸：1280×800（16:10），最小 1024×768
- 可全屏，保持 16:9 或 16:10 比例
- 图标：`apps/desktop/src-tauri/icons/`（占位图标）

### 自动更新

预留 Tauri updater 插件接口，当前返回 "无更新" 占位响应。
正式发布时配置更新服务器 URL 和签名密钥。

### 本地素材

桌面端素材目录（与 git 隔离）：
```
~/Library/Application Support/mamahjong/assets/
  characters/
  voices/
  effects/
  sounds/
```

客户端启动时检查素材目录是否存在，不存在则使用网页端同样的占位降级。
素材加载通过 Tauri `fs` API 或自定义协议 `tauri://asset/` 完成。

### 开发与构建

```bash
# 开发
cd apps/desktop && npm run tauri dev

# 构建 macOS .app
cd apps/desktop && npm run tauri build
```

构建产物不包含服务端，用户需自行启动服务端或连接远程服务器。
