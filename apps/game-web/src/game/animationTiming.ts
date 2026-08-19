/**
 * 玩家操作的动画时长（毫秒）。
 *
 * 后端 `mamahjong-application::presentation` 里有一份一模一样的表：一步操作的
 * 动画播完之后，下一家的读秒才开始计时，否则前端还在推副露，后端已经在扣人家
 * 的思考时间了。**改这里的数值必须同步改那边**，两边的常量名是对齐的。
 */

/** 吃/碰/杠/立直 横幅从弹出到淡出的整段时间。 */
export const CALL_BANNER_MS = 1300;
/** 副露从手牌边缘推到副露位。 */
export const MELD_PUSH_MS = 320;
/** 打出的牌从手里飞到牌河。 */
export const DISCARD_FLIGHT_MS = 400;
/** 动画收尾到下家开始读秒之间留的一点白。 */
export const ACTION_SETTLE_PADDING_MS = 120;

/**
 * 结算摊牌阶段的硬上界：摊手、翻役种、流局逐家亮牌都在这段里。
 *
 * 役种条目再多也必须在这个时刻之前进入结算读秒；服务端的兜底时限就是按这个
 * 上界推出来的。
 */
export const SETTLEMENT_REVEAL_BUDGET_MS = 12000;
/** 结算面板自动进入点数动画之前的读秒。 */
export const SETTLEMENT_COUNTDOWN_MS = 5000;
/**
 * 点棒增减演出的整段时长。
 *
 * 增减数字淡入、停一拍让人看清、浮上去贴到分数上、分数滚到位，全都在这段里；
 * 改 `PointChangeOverlay` 的节拍必须回头看这个数字够不够。
 */
export const POINTS_REVEAL_MS = 2800;
/** 换入牌交给二维手牌后的抬牌收束时长，动画只在换牌完成时播放一次。 */
export const EXCHANGE_INCOMING_SETTLE_MS = 1050;
/**
 * 确认窗口的倒计时。
 *
 * 这段读秒由服务端起算、剩余时间由服务端下发（`confirm_remaining_ms`），前端
 * 只负责显示，所以各家的按钮和数字完全同步。这里留一份只为了对表。
 */
export const SETTLEMENT_CONFIRM_MS = 5000;

/** 吃/碰/杠：横幅和推牌同时播，取长的那个。 */
export function meldCallAnimationMs(): number {
  return Math.max(CALL_BANNER_MS, MELD_PUSH_MS) + ACTION_SETTLE_PADDING_MS;
}

/** 加杠只是往已经摆好的碰上再叠一张，没有推牌动画，只等横幅。 */
export function addedKanAnimationMs(): number {
  return CALL_BANNER_MS + ACTION_SETTLE_PADDING_MS;
}

/** 普通打牌：牌飞到牌河为止。 */
export function discardAnimationMs(): number {
  return DISCARD_FLIGHT_MS + ACTION_SETTLE_PADDING_MS;
}

/** 立直宣言的那张牌：横幅和飞牌同时播。 */
export function riichiDiscardAnimationMs(): number {
  return Math.max(CALL_BANNER_MS, DISCARD_FLIGHT_MS) + ACTION_SETTLE_PADDING_MS;
}
