import { useEffect, useMemo, useState } from "react";
import { useGameStore } from "../stores/gameStore";
import type { MatchView, TileView } from "../types";
import { tableRelativeSeat } from "./table";
import { TilePlate } from "./TilePlate";
import { tileAssetPath } from "./tileAssets";

const fallbackAvatar =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

/** 某一家的听牌提示：听哪些牌、这几种一共还剩几枚。 */
export interface SeatWaitHint {
  waits: string[];
  remaining: number;
}

export function MatchHud({
  view,
  seatWaitHints,
}: {
  view: MatchView;
  /**
   * 牌谱重演的听牌提示。对局中不传——谁听什么是打牌时最该藏的信息。
   */
  seatWaitHints?: Map<number, SeatWaitHint>;
}) {
  const clocks = useGameStore((state) => state.clocks);
  const clockUpdatedAt = useGameStore((state) => state.clockUpdatedAt);
  const [now, setNow] = useState(Date.now());
  const seatCount = view.players.length;
  const activeSeat = turnSeat(view);
  // 冲击麻将没有宝牌、没有本场棒与场供棒，左上角只剩一张财神指示牌和连庄次数。
  const impact = view.variant_kind === "impact";
  const ownIsWaiting =
    activeSeat === view.observer_seat ||
    (view.phase.kind === "awaiting_responses" &&
      view.available_reactions.length > 0);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 100);
    return () => window.clearInterval(timer);
  }, []);

  const activeClock = ownIsWaiting
    ? clocks.get(view.observer_seat)
    : undefined;
  const visibleRemaining = useMemo(() => {
    if (!activeClock) return null;
    return Math.max(
      0,
      activeClock.remaining_ms - Math.max(0, now - clockUpdatedAt),
    );
  }, [activeClock, clockUpdatedAt, now]);

  return (
    <div className="match-hud" aria-label="对局信息">
      {impact ? (
        <aside className="match-dora-panel" aria-label="财神指示牌">
          <div className="match-dora-panel__tiles">
            <span>财神指示</span>
            <div>
              <DoraTile tile={view.joker_indicator} />
            </div>
          </div>
          <div className="match-dora-panel__counters">
            <span className="match-dora-panel__streak">
              连庄 {view.dealer_streak ?? 0} 次
            </span>
          </div>
        </aside>
      ) : (
        <aside className="match-dora-panel" aria-label="宝牌与场供">
          <div className="match-dora-panel__tiles">
            <span>宝牌指示</span>
            <div>
              {Array.from({ length: 5 }, (_, index) => {
                const tile = view.dora_indicators?.[index];
                return (
                  <DoraTile
                    key={tile?.id ?? `dora-back-${index}`}
                    tile={tile}
                  />
                );
              })}
            </div>
          </div>
          <div className="match-dora-panel__counters">
            <StickCounter
              count={view.progress.honba}
              value={100}
              label={`本场${view.progress.honba}`}
            />
            <StickCounter
              count={view.progress.riichi_sticks}
              value={1000}
              label={`场供${view.progress.riichi_sticks}`}
            />
          </div>
        </aside>
      )}

      {view.players.map((player) => {
        const relative =
          tableRelativeSeat(player.seat, view.observer_seat, seatCount);
        const active = player.seat === activeSeat;
        const riichi = player.riichi_status === "established";
        const waitHint = seatWaitHints?.get(player.seat);
        return (
          <section
            key={player.user_id}
            className={`match-player-panel match-player-panel--${relative}${
              active ? " is-active" : ""
            }${riichi ? " is-riichi" : ""}`}
          >
            <span className="match-player-panel__avatar-wrap">
              <img
                src={player.avatar_path ?? fallbackAvatar}
                alt=""
                onError={(event) => {
                  event.currentTarget.src = fallbackAvatar;
                }}
              />
              {riichi && (
                <span className="match-player-panel__riichi-badge">
                  立直
                </span>
              )}
            </span>
            <div className="match-player-panel__identity">
              <strong>{player.nickname}</strong>
              <span>
                {player.points.toLocaleString("zh-CN")}点
                {impact ? `（${player.kan_points ?? 0}）` : ""}
              </span>
            </div>
            {waitHint && waitHint.waits.length > 0 && (
              <WaitHintCard hint={waitHint} />
            )}
          </section>
        );
      })}
      {visibleRemaining != null && (
        <div
          className={`match-self-clock${
            visibleRemaining <= 5000 ? " is-urgent" : ""
          }`}
          role="timer"
          aria-label="我的剩余时间"
        >
          {formatClock(visibleRemaining)}
        </div>
      )}
    </div>
  );
}

/**
 * 听牌提示卡：贴在角色卡片下沿的一块小板，把听的牌一张张画出来，末尾跟剩余枚数。
 *
 * 牌用的就是主视角手牌那套实体牌（`TilePlate`），只是缩到很小一档——一眼能认出
 * 是牌、又不至于盖住牌桌。听十三面这种极端形会折成两排。
 */
function WaitHintCard({ hint }: { hint: SeatWaitHint }) {
  return (
    <div
      className="match-wait-card"
      aria-label={`听${hint.waits.length}种，还剩${hint.remaining}枚`}
    >
      <span className="match-wait-card__label" aria-hidden="true">
        听
      </span>
      <span className="match-wait-card__tiles" aria-hidden="true">
        {hint.waits.map((code) => (
          <TilePlate key={code} code={code} className="match-wait-card__tile" />
        ))}
      </span>
      <b className="match-wait-card__count" aria-hidden="true">
        {hint.remaining}
      </b>
    </div>
  );
}

function StickCounter({
  count,
  value,
  label,
}: {
  count: number;
  value: 100 | 1000;
  label: string;
}) {
  return (
    <span className="match-stick-counter" aria-label={label}>
      <strong>{count}</strong>
      <i
        className={`match-point-stick match-point-stick--${value}`}
        aria-hidden="true"
      />
    </span>
  );
}

function DoraTile({ tile }: { tile?: TileView }) {
  return (
    <span className={`match-dora-slot${tile ? "" : " is-back"}`}>
      {tile && (
        <img
          className="match-dora-tile"
          src={tileAssetPath(tile.code, "jp")}
          alt=""
        />
      )}
    </span>
  );
}

function turnSeat(view: MatchView): number | null {
  if (
    view.phase.kind === "awaiting_turn_action" ||
    view.phase.kind === "awaiting_discard"
  ) {
    return view.phase.seat;
  }
  return null;
}

function formatClock(milliseconds: number): string {
  return `${Math.ceil(milliseconds / 1000)}秒`;
}
