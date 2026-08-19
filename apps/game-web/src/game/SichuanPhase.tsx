import { useEffect, useMemo, useState } from "react";
import type { GameCommandName, MatchView, TileView } from "../types";
import type { OpeningPhase } from "./OpeningSequence";
import { tileSuitCode } from "./PrePlayPanels";
import { sortHandForDisplay } from "./table";
import { tileAssetPath } from "./tileAssets";
import { DingQuePanel } from "./DingQuePanel";

/**
 * 四川麻将开打前的两段流程：换三张与定缺。
 *
 * 换三张的**选牌**在二维手牌里完成（`SichuanExchangeHand`）；点「换牌」提交后，
 * 整段飞出/换位/飞入的演出交给三维牌桌（见 `table/exchange.ts`），这里只剩浮在
 * 桌上方的提示与定缺面板。选牌规则与提交快照由 GameScene 掌管。
 */
export function SichuanPhaseOverlay({
  view,
  openingPhase,
  onCommand,
  onConfirmExchange,
  exchangeLocallySubmitted,
  exchangeAnimationDone,
}: {
  view: MatchView;
  openingPhase: OpeningPhase;
  onCommand: (name: GameCommandName, payload?: unknown) => void;
  onConfirmExchange: (tileIds: number[]) => void;
  /** 本地已点「换牌」。服务端已交名单还在路上时，也按已交谈。 */
  exchangeLocallySubmitted: boolean;
  /** 后端进入定缺即代表四家动画已回执或已走超时兜底。 */
  exchangeAnimationDone: boolean;
}) {
  const isExchange = view.phase.kind === "awaiting_exchange";
  const isExchangeAnimation =
    view.phase.kind === "awaiting_exchange_animation";
  const isDingque = view.phase.kind === "awaiting_dingque";
  const observerSeat = view.observer_seat;
  const exchangeSubmitted =
    (view.exchange_submitted_seats?.includes(observerSeat) ?? false) ||
    exchangeLocallySubmitted;
  const dingqueSubmitted =
    view.dingque_submitted_seats?.includes(observerSeat) ?? false;
  const observer = view.players.find((player) => player.seat === observerSeat);
  const ownTiles = observer?.concealed_tiles ?? [];
  const [phaseDeadline, setPhaseDeadline] = useState(
    () => Date.now() + (view.phase_remaining_ms ?? 0),
  );
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    setPhaseDeadline(Date.now() + (view.phase_remaining_ms ?? 0));
  }, [view.phase.kind, view.phase_remaining_ms]);

  useEffect(() => {
    if (view.phase.kind !== "awaiting_exchange" && view.phase.kind !== "awaiting_dingque") {
      return;
    }
    const timer = window.setInterval(() => setNow(Date.now()), 200);
    return () => window.clearInterval(timer);
  }, [view.phase.kind]);

  const phaseSeconds = Math.max(0, Math.ceil((phaseDeadline - now) / 1000));

  if (
    openingPhase !== "play" ||
    (!isExchange && !isExchangeAnimation && !isDingque)
  )
    return null;

  /* 换三张：未交时在二维手牌上挑；交了之后三维只演飞出的牌，这里只留提示。 */
  if (isExchange) {
    return (
      <div className="match-sichuan-phase">
        <div
          className={`match-self-clock${phaseSeconds <= 5 ? " is-urgent" : ""}`}
          role="timer"
          aria-label="我的剩余时间"
        >
          {phaseSeconds}秒
        </div>
        {exchangeSubmitted ? (
          <div className="match-sichuan-status">等待其他人选择换牌</div>
        ) : (
          <SichuanExchangeHand
            tiles={ownTiles}
            onConfirm={onConfirmExchange}
          />
        )}
      </div>
    );
  }

  if (isExchangeAnimation) {
    /* 三维换牌动画期间不再叠加状态框或方向文字，避免遮挡牌桌演出。 */
    return null;
  }

  /*
   * `awaiting_dingque` 是服务端唯一的定缺闸门。换牌动画回执只是动画同步状态，
   * 不能再拿它决定是否挂载面板：断线重连或客户端没有三维演出时，回执名单可能
   * 还没补齐，但服务端已经明确进入了定缺阶段。
   */
  return (
    <div className="match-sichuan-phase">
      <div
        className={`match-self-clock${phaseSeconds <= 5 ? " is-urgent" : ""}`}
        role="timer"
        aria-label="我的剩余时间"
      >
        {phaseSeconds}秒
      </div>
      {dingqueSubmitted ? (
        <div className="match-sichuan-status">等待其他人完成定缺</div>
      ) : (
        <DingQuePanel
          onSelect={(suit) => onCommand("sichuan.ding_que", { suit })}
        />
      )}
    </div>
  );
}

/**
 * 在原手牌（二维）上挑 3 张同花色的牌交出去。
 *
 * 灰牌规则：不足 3 张的花色直接变灰；先挑定某一门后其余门变灰；挑满 3 张后剩余
 * 的牌变灰。选中的牌上浮描红边，再点一下取消。点「换牌」把选中的 id 交上去。
 */
function SichuanExchangeHand({
  tiles,
  onConfirm,
}: {
  tiles: TileView[];
  onConfirm: (tileIds: number[]) => void;
}) {
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const sorted = useMemo(() => sortHandForDisplay(tiles, null), [tiles]);
  const suitCounts = useMemo(() => {
    const counts: Record<"m" | "p" | "s", number> = { m: 0, p: 0, s: 0 };
    for (const tile of sorted) {
      const suit = tileSuitCode(tile.code);
      if (suit) counts[suit] += 1;
    }
    return counts;
  }, [sorted]);
  const selectedSuit = useMemo(() => {
    const firstId = selectedIds[0];
    if (firstId == null) return null;
    const first = sorted.find((tile) => tile.id === firstId);
    return first ? tileSuitCode(first.code) : null;
  }, [selectedIds, sorted]);

  const canSelect = (tile: TileView): boolean => {
    if (selectedIds.length >= 3) return false;
    const suit = tileSuitCode(tile.code);
    if (!suit) return false;
    if (suitCounts[suit] < 3) return false;
    if (selectedSuit != null && suit !== selectedSuit) return false;
    return true;
  };

  const toggle = (tile: TileView) => {
    if (selectedIds.includes(tile.id)) {
      setSelectedIds((current) => current.filter((id) => id !== tile.id));
      return;
    }
    if (!canSelect(tile)) return;
    setSelectedIds((current) => [...current, tile.id]);
  };

  return (
    <div className="match-sichuan-exchange" aria-label="换三张">
      <div className="match-hand-2d">
        {sorted.map((tile) => {
          const selected = selectedIds.includes(tile.id);
          const blocked = !selected && !canSelect(tile);
          return (
            <button
              key={tile.id}
              type="button"
              className={`tile-plate match-hand-2d__tile${
                selected ? " is-exchange-selected" : ""
              }${blocked ? " is-exchange-blocked" : ""}`}
              aria-pressed={selected}
              onClick={() => toggle(tile)}
            >
              <span className="tile-plate__body match-hand-2d__body">
                <span className="tile-plate__face match-hand-2d__face">
                  <img src={tileAssetPath(tile.code, "jp")} alt="" />
                </span>
              </span>
            </button>
          );
        })}
      </div>
      <div className="match-sichuan-exchange__bar">
        <span className="match-sichuan-exchange__hint">
          请选择3张同花色牌换牌
        </span>
        <button
          type="button"
          className="match-brush-button"
          disabled={selectedIds.length !== 3}
          onClick={() => {
            if (selectedIds.length === 3) onConfirm(selectedIds);
          }}
        >
          换牌
        </button>
      </div>
    </div>
  );
}

export const DIRECTION_LABELS: Record<
  "counter_clockwise" | "clockwise" | "opposite",
  string
> = {
  counter_clockwise: "逆时针",
  clockwise: "顺时针",
  opposite: "对家",
};
