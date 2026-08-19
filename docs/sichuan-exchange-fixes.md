# 四川麻将换三张与摸打修复设计

## 目标

1. 换牌选牌在原手牌上操作：选中上浮、一个「换牌」按钮、不弹独立窗口。
2. 换牌发生在各家摸完初始牌之后（庄家第 14 张先摸，再换三张）。
3. 显示换牌动画。
4. 换完牌后庄家正常出牌、其余各家摸牌后能正常打牌。

## 后端（crates/mahjong-sichuan/src/hand/state.rs）

- `SichuanHand::new`：发完各家 13 张后，庄家先摸第 14 张（只 `insert`，不记 `drawn`），
  `turns_taken = 1`，阶段仍为 `AwaitingExchange`。
- `complete_exchange`：交换完成后置 `interrupted = true`——换牌改了开局手牌，天胡/地胡作废。
- `start_play`：不再 `draw_and_open_turn`，直接 `phase = AwaitingTurnAction { dealer }`。

## 前端

- `SichuanPhase`：换三张改为原手牌选牌（新组件 `SichuanExchangeHand`），去掉独立窗口
  `TileExchangePanel`；定缺仍走 `ChoicePanel`；换三张方向大字横幅保留作换牌动画。
- 换三张与定缺只在开局动画放行（`openingPhase === "play"`）后才显示，避免「一开始就弄」。
- `PlayerHand2D`：手上有定缺门时，非定缺门的牌变灰不可打（对齐后端 `QueTilesRemaining`）。
- `MatchHud`：`ownIsWaiting` 补上换三张与定缺两阶段，选牌时像平时操作一样走自己的倒计时。
- 换三张交完后若还有人没交，显示「等待其他人完成换牌」。
