import type { CSSProperties } from "react";
import type { MatchView } from "../types";
import { tableRelativeSeat } from "./table";
import { CALL_LABELS, type CallBannerItem } from "./callBanners";
import { WaitingTilesPanel } from "./WaitingTilesPanel";

interface CallBannerLayerProps {
  view: MatchView;
  banners: CallBannerItem[];
}

/**
 * Shouts 吃 / 碰 / 杠 / 和了 next to the hand of whoever acted. Seatless
 * banners (流局) land in the middle of the table instead.
 */
export function CallBannerLayer({ view, banners }: CallBannerLayerProps) {
  if (banners.length === 0) return null;

  return (
    <div className="call-banner-layer" aria-live="polite">
      {banners.map((banner) => {
        const label = banner.label ?? CALL_LABELS[banner.kind];
        const relative =
          banner.seat == null
            ? null
            : tableRelativeSeat(
                banner.seat,
                view.observer_seat,
                view.players.length,
              );
        const waits = banner.waits ?? [];
        return (
          <div
            key={banner.id}
            className={`call-banner call-banner--${banner.kind} ${
              relative == null ? "is-center" : `is-seat-${relative}`
            }${banner.holdMs ? " is-hold" : ""}${
              waits.length > 0 ? " is-waits" : ""
            }`}
            style={
              banner.holdMs
                ? ({ "--call-life": `${banner.holdMs}ms` } as CSSProperties)
                : undefined
            }
          >
            <div className="call-banner__inner">
              {waits.length > 0 ? (
                /* 流局摊牌看的就是听什么牌，直接借自己那套听牌框，
                   不走鸣牌的笔刷，也不标有役无役。 */
                <WaitingTilesPanel
                  waitingTiles={waits}
                  className="call-banner__waits"
                />
              ) : (
                /* 先甩一笔笔刷，字再砸上去。 */
                <span className="call-banner__shout">
                  <span className="call-banner__brush" aria-hidden="true" />
                  <b className="call-banner__text">{label}</b>
                </span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
