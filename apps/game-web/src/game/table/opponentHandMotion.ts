/**
 * 对手暗手在桌面上占用的槽位。
 *
 * `renderedSlots` 的长度永远等于服务端给出的真实手牌数；槽位可以比牌数多一个，
 * 那个多出来的位置就是手切留下的空位。这样动画只改变位置，不会靠增删牌来伪造。
 */
export interface OpponentHandLayout {
  slotCount: number;
  renderedSlots: number[];
  drawnSlot: number | null;
}

export function opponentHandLayout(
  concealedTileCount: number,
  holdingDrawnTile: boolean,
  handCutGap?: number,
): OpponentHandLayout {
  const count = Math.max(0, concealedTileCount);

  /*
   * 手切后的 count 已经是 3k+1：其中一张是刚才摸进来的牌。基础牌阵占 count 个
   * 槽，切掉的位置留空；摸入牌继续独立放在最右边的第 count 个槽。
   */
  if (handCutGap != null && count > 0) {
    const gap = Math.min(count - 1, Math.max(0, handCutGap));
    return {
      slotCount: count + 1,
      renderedSlots: [
        ...Array.from({ length: count }, (_, index) => index).filter(
          (index) => index !== gap,
        ),
        count,
      ],
      drawnSlot: count,
    };
  }

  return {
    slotCount: count,
    renderedSlots: Array.from({ length: count }, (_, index) => index),
    drawnSlot: holdingDrawnTile && count > 0 ? count - 1 : null,
  };
}

