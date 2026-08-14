import { disposeGroup } from "./runtime";
import type { TableRuntime, TransientTile } from "./types";

/**
 * 只登台一会儿的牌：演完那一下就地拆掉。
 *
 * 桌上其余的牌都有稳定的视图位置；这类牌在视图里根本没有对应的位置（自己摸的
 * 那张牌落地之后归二维手牌管），留在场上就成了穿帮的重影，所以到点必须自己走。
 */
export function advanceTransientTiles(
  runtime: TableRuntime,
  now: number,
): TransientTile[] {
  return runtime.transients.filter((transient) => {
    if (now < transient.removeAt) return true;
    transient.group.removeFromParent();
    disposeGroup(transient.group);
    return false;
  });
}
