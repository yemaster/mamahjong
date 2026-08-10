import { useEffect, useMemo, useRef, useState } from "react";
import type { CallBannerItem } from "../game/callBanners";
import { settlementCoveringSeats, TSUMO_THROW_MS } from "../game/table";
import type { MatchView } from "../types";

/**
 * 重演里的和牌动画。
 *
 * 对局中那条时间线（`GameScene`）后面还挂着确认窗口、读秒和点数动画——那些都是四家
 * 要对齐的握手，重演没有对手要等，所以这里只留看得见的那三拍：
 *
 * 1. 和牌那家旁边喊一声「自摸／荣和」；
 * 2. 自摸先把那张牌砸到桌上，再把手牌摊平；点炮的（或自摸时其余三家）跟着翻开；
 * 3. 手牌都摊完了才升起结算面板。
 *
 * 走到别的一步、或者用户把面板收了，整条就地重置——重演可以随时往回退，动画不能
 * 卡在半截上。
 */

/** 面板亮起来之前的三拍加起来有多长，用来给自动播放让路。 */
export interface ReplaySettlementTimeline {
  /** 手牌已经摊平的座位，直接喂给 `GameTable`。 */
  revealSeats: number[];
  /** 自摸那张牌已经翻起来的座位。 */
  winningTileSeats: number[];
  /** 结算面板该不该在屏幕上。 */
  panelVisible: boolean;
  /** 喊出来的那几声，喂给 `CallBannerLayer`。 */
  banners: CallBannerItem[];
  /** 用户点了「收起」之后把面板收掉，动画不重播。 */
  dismissPanel: () => void;
}

/** 喊声挂多久。 */
const BANNER_MS = 1600;

export function useReplaySettlement(
  view: MatchView | null,
  /** 走到别的一步就换个值，整条时间线跟着重来。 */
  stepKey: string,
): ReplaySettlementTimeline {
  const [revealSeats, setRevealSeats] = useState<number[]>([]);
  const [winningTileSeats, setWinningTileSeats] = useState<number[]>([]);
  const [panelVisible, setPanelVisible] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [banners, setBanners] = useState<CallBannerItem[]>([]);
  const timers = useRef<number[]>([]);

  /* 结算本身认的是「哪一局的哪一种结果」，跟走到第几步无关。 */
  const settlement = view?.hand_settlement ?? null;
  const key = settlement ? stepKey : "";

  useEffect(() => {
    timers.current.forEach(window.clearTimeout);
    timers.current = [];
    setRevealSeats([]);
    setWinningTileSeats([]);
    setPanelVisible(false);
    setDismissed(false);
    setBanners([]);
    if (!view || !settlement || settlement.winners.length === 0) return;

    const at = (ms: number, run: () => void) => {
      timers.current.push(window.setTimeout(run, ms));
    };
    const revealSeat = (seat: number) =>
      setRevealSeats((current) =>
        current.includes(seat) ? current : [...current, seat],
      );

    const isTsumo = settlement.reason === "tsumo";
    const winnerSeats = settlement.winners.map((winner) => winner.seat);

    /* 一、喊声。 */
    winnerSeats.forEach((seat, index) => {
      const id = `${isTsumo ? "tsumo" : "ron"}-${seat}`;
      at(index * 240, () =>
        setBanners((current) => [
          ...current,
          { id, kind: isTsumo ? "tsumo" : "ron", seat, label: undefined },
        ]),
      );
      at(index * 240 + BANNER_MS, () =>
        setBanners((current) => current.filter((banner) => banner.id !== id)),
      );
    });

    /* 二、自摸的那张牌先落桌，等这一下砸完手牌才瘫下去。 */
    let handRevealAt = 300;
    if (isTsumo) {
      winnerSeats.forEach((seat, index) => {
        at(handRevealAt + index * 240, () =>
          setWinningTileSeats((current) =>
            current.includes(seat) ? current : [...current, seat],
          ),
        );
      });
      handRevealAt += TSUMO_THROW_MS + 200;
    }
    winnerSeats.forEach((seat, index) => {
      at(handRevealAt + index * 320, () => revealSeat(seat));
    });
    const winnerRevealEnd = handRevealAt + winnerSeats.length * 320;

    /* 荣和只有点炮的那家翻牌，自摸是其余三家一起翻。 */
    const covering = settlementCoveringSeats(view);
    if (covering.length > 0) {
      at(winnerRevealEnd + 220, () =>
        setRevealSeats((current) => {
          const next = [...current];
          for (const seat of covering) if (!next.includes(seat)) next.push(seat);
          return next;
        }),
      );
    }

    /* 三、面板。时序照抄对局，只是后面不再接读秒和点数动画。 */
    at(winnerRevealEnd + 220 + 340 + 900, () => setPanelVisible(true));

    return () => {
      timers.current.forEach(window.clearTimeout);
      timers.current = [];
    };
    /*
     * 只认 `key`：`view` 每一步都是新对象，挂进依赖会让动画每步重播一遍。
     * 真正会换结算的只有换局和换步，那两样都写进 `key` 里了。
     */
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return useMemo(
    () => ({
      revealSeats,
      winningTileSeats,
      panelVisible: panelVisible && !dismissed,
      banners,
      dismissPanel: () => setDismissed(true),
    }),
    [revealSeats, winningTileSeats, panelVisible, dismissed, banners],
  );
}
