# 牌谱与重演

牌谱是一局对战打完之后留下的那份完整记录：每一张摸的牌、每一次鸣牌、每一次立直
都在里面。服务端一直在写，这份文档说明它的数据形状、下发规则，以及客户端怎么把它
还原成一张能一步步往前走的牌桌。

## 一、牌谱的产生与归档

- `MatchRecord`（`crates/mamahjong-application/src/record.rs`）由 `GameRuntime` 现场生成，
  schema 固定为 `match_record.v1`。新增字段一律追加，不改 schema 号：旧牌谱少几个字段，
  客户端按缺省处理，不能因此解析失败。
- 每次指令推进对局后 `MatchArchive::persist` 把整份牌谱写到 `{归档目录}/{match_id}.json`
  （临时文件 + `rename` + 目录 `fsync`）。内存里的 `store.matches` 在服务端重启后就没了，
  **牌谱的唯一长期来源是归档目录**。

## 二、牌山快照

- 一局的牌山顺序在 `Wall` 里始终完整：摸牌只推进游标，`tiles` 数组从不改动。
- `GameRuntime` 在每次开新一局（`start` 与 `advance_settlement`）时，把这一局的牌山
  顺序连同活牌区末尾一起存进 `hand_walls`。
- `HandRecord.wall` 的形状：

  ```json
  { "tiles": [{ "id": 0, "code": "3m" }, ...], "live_end": 69 }
  ```

  `tiles[0..live_end]` 是活牌区（按摸牌顺序排），`tiles[live_end..live_end + 14]` 是王牌。

- **牌山只在对局结束后下发。** `MatchRecord::from_runtime` 只有在
  `runtime.game.result().is_some()` 时才填 `wall`，否则每一局都是 `null`。
  进行中的对局把牌山发给任何一个客户端都等于送作弊器，这条没有例外，
  也不许用「只发已经打完的那几局」之类的折中放宽。
- 牌山种子（`WallSeed`）永远不出服务端：牌谱里存的是展开后的牌序，不是种子。

## 三、牌谱接口

| 接口 | 用途 |
| --- | --- |
| `GET /records` | 当前用户的对局记录列表，按结束时间倒序 |
| `GET /matches/{match_id}/record` | 单局完整牌谱 |

- 两个接口都要求请求者本人曾在这局里坐过一个位置；归档回落路径同样按
  `players[].user_id` 校验，不能因为绕开了内存那条路就少一道检查。
- `GET /matches/{id}/record` 先查内存 `store.matches`，未命中再读归档文件。
  只查内存的话，服务端一重启历史对局就全部 404，牌谱页会变成一片空白。
- `GET /records` 扫归档目录，只收 `result` 非空（也就是真的打完）的牌谱，每条产出：

  ```
  match_id, finished_at_ms, friend_match, variant, match_length, rule_name, hand_count,
  seats: [{ seat, nickname, rank, points, score_tenths }]
  ```

- 列表上的「点数增减」写的是 `score_tenths`（十分之一点为单位的最终得分），
  **不是 `points` 减起始点数**：一局的胜负是算过返点、马点和 oka 之后的结果，
  `score_tenths = (素点 − 返点) / 100 + 马点 + oka`，四家加起来是零和。
  直接相减出来的差值和终局结算页面上的数字对不上，也没法反推马点，所以马点算完的
  结果必须落在牌谱里——这份记录不留原始马点配置，留的是算完的得分。

### 规则名怎么来的

牌谱标题写三段：`好友对战 · 四人南 · ML规则`。第三段是**读的时候现算的**，不存进归档
（`mamahjong_application::rule_display_name`，两个接口共用一个函数）：

- 快照挂着预设、配置和那份预设一字不差 → 预设短名（`RiichiPreset::short_name`：
  「A规」「最高位战」「ML规则」）。
- 快照挂着预设、配置被改过 → 「自定义规则」。**光看有没有预设引用判断不出改没改过**：
  `ResolvedRiichiRules` 应用覆盖之后仍然留着出处，所以必须拿配置本身跟 `preset.rules()` 比。
- 没挂预设、和该人数的标准规则一致 → 「标准规则」；否则同样是「自定义规则」。

段位匹配用的就是标准规则，前端在拼标题时把这一段整个省掉——「段位匹配」四个字本身
已经把规则说清楚了，后面再挂一句「标准规则」是重复。

规则名不写进归档是有意的：预设改版之后，磁盘上存死的名字就成了旧账，而现算的永远
是拿当下的预设定义比出来的。

- `finished_at_ms` 取归档文件的修改时间。应用层只有一个从进程启动算起的单调时钟
  （`MonotonicClock`，见 `apps/server/src/clock.rs`），那个数拿来当时间戳显示就是 1970 年；
  归档写完的时刻就是这局打完的时刻，列表排序和显示都够用了。牌谱本身不带时间戳。
- 目录扫描是阻塞 IO，和 `player_statistics` 一样走 `spawn_blocking`。

## 四、和牌的结算明细

重演要把和牌那一屏原样播一遍（演出节奏见 `docs/client/visual-spec.md` 的牌谱一节），
所以每一局的结算数据得齐：

- `HandRecord.winner_scores` 一家和牌一条，和 `winners` 同序：
  `{ seat, han, fu, yakuman_multiplier, limit, points, dealer, yaku: [{ name, value, yakuman }] }`。
  番符和役种本来就在 `HandResult.winners()` 的 `WinEvaluation` 里，只是以前没写出去。
- `HandRecord.ura_dora_indicators` 是这一局翻出来的里宝牌。它只在结算那一刻算得出来
  （要用当时的手牌状态），所以 `GameRuntime.finish_hand` 在 `apply_hand` 之后就地存进
  `hand_ura_dora`；**对局中的观战视图读的也是这一份**，牌谱和实时画面不可能对不上。
- **流局两样都是空**：没人和牌就没有番符，也不翻里宝牌。前端据此判断这一局不结算。
- **缺番符只是不开面板，演出照演。** `winner_scores` 是后来加的字段，归档一写完就冻住，
  之前打的每一局都补不回来。所以 `replaySettlement.ts` 把结算拆成两件事：谁和的、
  怎么和的（`winners` / `reason` / `from`）事件日志里本来就有，喊声、砸牌、摊手牌照播；
  番符役种缺了才关掉面板（`ReplaySettlement.detailed` 为假）。整段不播是不对的——
  用户要看的就是和牌那一下。
- 役种名、场风名、结束原因名、番种上限名这四张表在 `dto.rs`（实时接口）和
  `record.rs`（牌谱）两处都要用，统一放在 `crates/mamahjong-application/src/naming.rs`。
  两边必须吐出一模一样的字符串，否则同一局在牌桌上和牌谱里会写出两个名字。

## 五、旧牌谱的降级

已经归档在磁盘上的牌谱没有牌山、也没有好友标记。它们必须照样能用：

- 列表照常列出。时间本来就取文件修改时间，不受影响；`friend_match` 缺失时
  标题只写规则部分（「四人南」），不猜是好友还是匹配。
- `winner_scores` 缺失的那几局只演不算（见上一节）。
- `riichi.tile_discarded` 的 `tsumogiri` 缺失时一律当手切，牌河里不压暗：宁可不标，
  也不能凭空给一张牌安上摸切。
- 重演照常进入，跳局、跳巡、单步、自动播放、摊牌、听牌与铳牌提示全部可用——这些
  只依赖事件日志。
- 只有牌山面板降级，写明「该对局无牌山记录」。

## 六、事件日志折叠成牌桌状态

客户端不另写一套牌桌，而是把牌谱折叠成一份合成的 `MatchView`，交给正式对局那套
`GameTable` / `MatchHud` / `PlayerHand2D` 渲染（`TableSettingsScene` 已经证明这条路走得通）。

- 每一局从局首开始，按 `HandRecord.events` 顺序重放到第 N 步，得到四家的暗手、副露、
  牌河、立直状态，以及宝牌指示牌与余牌数。牌谱里每一家的牌都是明的，所以任何一步的
  牌桌状态都能完整还原，不需要服务端再算一遍。
- 一步就是一个事件。巡数按庄家每摸一次牌 +1。
- 合成视图每走一步必须递增 `version`：牌桌的重绘只认 `view.id`、`view.hand_index`
  和 `view.version` 这三样，`version` 不动的话画面不会跟着走。
- 观察者座位可以任意切换，切换只改 `observer_seat` 和重算一次视图，不重建牌桌。

## 七、听牌判定在前端

- 牌谱重演里的听牌、铳牌提示由前端的 TS 求解器算（`apps/game-web/src/replay/waits.ts`），
  标准型、七对子、国士三种都要覆盖。拖进度条时每一步都要重算，来回请求服务端撑不住。
- 服务端的 `WinQuery` 仍然是正式对局的唯一判定来源。两份实现只在牌谱这条路上并存，
  前端那份不参与任何正式对局的判定，也不允许反过来被正式对局引用。
- 剩余枚数的口径：`4 × 听牌种数 − 已经看得见的同种牌`，看得见的包括四家手牌、
  所有副露和所有牌河。赤五和普通五算同一种牌。
