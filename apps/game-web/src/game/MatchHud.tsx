import { useEffect, useState } from "react";
import { useGameStore } from "../stores/gameStore";
import type { MatchView, TileView } from "../types";
import { tableRelativeSeat } from "./table";
import { TilePlate } from "./TilePlate";
import { tileAssetPath } from "./tileAssets";

const fallbackAvatar =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

/** 四川麻将的定缺门中文名，徽章只写一个字。 */
const QUE_SUIT_LABELS: Record<string, string> = {
  man: "万",
  pin: "筒",
  sou: "索",
};

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
  const seatCount = view.players.length;
  const activeSeat = turnSeat(view);
  // 冲击麻将没有宝牌、没有本场棒与场供棒，左上角只剩一张财神指示牌和连庄次数。
  const impact = view.variant_kind === "impact";
  // 四川麻将连这些都没有：左上角什么都不摆。
  const sichuan = view.variant_kind === "sichuan";
  const ownIsWaiting =
    activeSeat === view.observer_seat ||
    (view.phase.kind === "awaiting_responses" &&
      view.available_reactions.length > 0);

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
      ) : sichuan ? null : (
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
              {player.que_suit && (
                <span className="match-player-panel__que-badge">
                  缺{QUE_SUIT_LABELS[player.que_suit]}
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
      <SelfClock seat={view.observer_seat} active={ownIsWaiting} />
    </div>
  );
}

/** 高频倒计时独立渲染，避免秒数刷新带着整块 HUD、头像和宝牌一起重绘。 */
function SelfClock({ seat, active }: { seat: number; active: boolean }) {
  const clock = useGameStore((state) =>
    active ? state.clocks.get(seat) : undefined,
  );
  const clockUpdatedAt = useGameStore((state) => state.clockUpdatedAt);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (!clock) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [clock, clockUpdatedAt]);

  if (!clock) return null;
  const visibleRemaining = Math.max(
    0,
    clock.remaining_ms - Math.max(0, now - clockUpdatedAt),
  );
  return (
    <div
      className={`match-self-clock${
        visibleRemaining <= 5000 ? " is-urgent" : ""
      }`}
      role="timer"
      aria-label="我的剩余时间"
    >
      {formatClock(visibleRemaining)}
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
