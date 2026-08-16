import { useCallback, useEffect, useState, type CSSProperties } from "react";
import type { MatchView, TileView, WinnerSettlementView } from "../types";
import { isDoraTile, tileAssetPath } from "./tileAssets";
import { isJokerTile, sortHandForDisplay } from "./table";
import { HULE_FAN_OUT_SFX, FU_APPEAR_SFX, SCORE_APPEAR_SFX, playSfxAndWait } from "../audio/sfx";

interface HandSettlementProps {
  view: MatchView;
  showPanel: boolean;
  confirmReady: boolean;
  secondsRemaining: number;
  locallyConfirmed: boolean;
  onConfirm: () => void;
  /**
   * 最后一位的按钮文字，默认是「确定 {读秒}」。
   *
   * 牌谱重演没有对手要等、也没有读秒，写「收起」——按下去只是把面板收掉。
   */
  confirmLabel?: string;
}

const delay = (ms: number) => new Promise<void>((r) => window.setTimeout(r, ms));

export function HandSettlement({
  view,
  showPanel,
  confirmReady,
  secondsRemaining,
  locallyConfirmed,
  onConfirm,
  confirmLabel,
}: HandSettlementProps) {
  const settlement = view.hand_settlement;
  if (!settlement) return null;

  const selfConfirmed =
    locallyConfirmed || settlement.confirmed_seats.includes(view.observer_seat);

  /* 流局 never opens a board — the hands reveal one by one on the table and
     the point animation follows straight after. */
  if (!showPanel || settlement.winners.length === 0) return null;

  return (
    <WinScreen
      view={view}
      confirmReady={confirmReady}
      secondsRemaining={secondsRemaining}
      selfConfirmed={selfConfirmed}
      onConfirm={onConfirm}
      confirmLabel={confirmLabel}
    />
  );
}

/* ─────────────────────────── 和了 ─────────────────────────── */

function WinScreen({
  view,
  confirmReady,
  secondsRemaining,
  selfConfirmed,
  onConfirm,
  confirmLabel,
}: {
  view: MatchView;
  confirmReady: boolean;
  secondsRemaining: number;
  selfConfirmed: boolean;
  onConfirm: () => void;
  confirmLabel?: string;
}) {
  const settlement = view.hand_settlement!;
  const winners = settlement.winners;
  const [active, setActive] = useState(0);
  const [boardReady, setBoardReady] = useState(false);

  /* 每换一个和了者就重置。 */
  const onBoardReady = useCallback(() => setBoardReady(true), []);

  useEffect(() => {
    setBoardReady(false);
  }, [active]);

  const index = Math.min(active, winners.length - 1);
  const winner = winners[index];
  const player = view.players.find((entry) => entry.seat === winner?.seat);
  if (!winner || !player) return null;

  const isLast = index >= winners.length - 1;
  const canConfirm = boardReady && confirmReady;

  return (
    <section className="win-screen" aria-label="和了结算">
      <WinBoard
        key={winner.seat}
        view={view}
        winner={winner}
        player={player}
        onReady={onBoardReady}
      />

      {winners.length > 1 && (
        <div className="win-screen__pager">
          {winners.map((entry, i) => (
            <i key={entry.seat} className={i === index ? "is-active" : ""} />
          ))}
        </div>
      )}

      {boardReady && confirmReady && (
        <button
          className="win-screen__confirm"
          type="button"
          disabled={isLast && selfConfirmed}
          onClick={() => {
            if (isLast) onConfirm();
            else setActive(index + 1);
          }}
        >
          {isLast
            ? selfConfirmed
              ? "已确定"
              : (confirmLabel ?? `确定 ${secondsRemaining}`)
            : "下一位"}
        </button>
      )}
    </section>
  );
}

type RevealPhase = "yaku" | "tally" | "score" | "done";

function WinBoard({
  view,
  winner,
  player,
  onReady,
}: {
  view: MatchView;
  winner: WinnerSettlementView;
  player: MatchView["players"][number];
  /** 动画序列全部播完之后回调，上层用它在一切就绪后才露出确认按钮。 */
  onReady: () => void;
}) {
  const settlement = view.hand_settlement!;
  const isTsumo = settlement.reason === "tsumo";
  const impact = view.variant_kind === "impact";
  const allIn = impact ? (settlement.all_in ?? null) : null;
  const doraIndicators = view.dora_indicators ?? [];
  const uraIndicators = settlement.ura_dora_indicators ?? [];
  const riichi = player.riichi_status === "established";

  const [phase, setPhase] = useState<RevealPhase>("yaku");
  const [yakuCount, setYakuCount] = useState(0);
  const [showTally, setShowTally] = useState(false);
  const [showScore, setShowScore] = useState(false);

  const yakuLen = winner.yaku.length;

  /* ── 番种逐个亮出 ── */
  useEffect(() => {
    if (phase !== "yaku") return;
    let cancelled = false;

    const run = async () => {
      for (let i = 0; i < yakuLen; i++) {
        if (cancelled) return;
        // 第一条番种等一瞬再出，让面板先到位
        if (i === 0) await delay(320);
        if (cancelled) return;

        setYakuCount(i + 1);
        await playSfxAndWait(HULE_FAN_OUT_SFX);
        if (cancelled) return;
        await delay(250);
      }
      if (cancelled) return;
      // 全部番种亮完 → 冲击麻将直接跳点数，立直麻将先亮番符
      setPhase(impact ? "score" : "tally");
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [phase, yakuLen, impact]);

  /* ── 番符出现（仅立直麻将） ── */
  useEffect(() => {
    if (phase !== "tally") return;
    let cancelled = false;

    const run = async () => {
      setShowTally(true);
      await playSfxAndWait(FU_APPEAR_SFX);
      if (cancelled) return;
      await delay(250);
      if (cancelled) return;
      setPhase("score");
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [phase]);

  /* ── 总得点出现 ── */
  useEffect(() => {
    if (phase !== "score") return;
    let cancelled = false;

    const run = async () => {
      setShowScore(true);
      await playSfxAndWait(SCORE_APPEAR_SFX);
      if (cancelled) return;
      setPhase("done");
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [phase]);

  /* ── 全部完成 ── */
  useEffect(() => {
    if (phase !== "done") return;
    onReady();
  }, [phase, onReady]);

  // Winning tile: tsumo → the drawn tile; ron → the dealt-in player's last discard.
  let winningTile: TileView | null = null;
  if (isTsumo) {
    winningTile =
      (player.concealed_tiles ?? []).find(
        (tile) => tile.id === player.drawn_tile_id,
      ) ?? null;
  } else if (settlement.from_seat != null) {
    const dealer = view.players.find(
      (entry) => entry.seat === settlement.from_seat,
    );
    winningTile = dealer?.discards.at(-1)?.tile ?? null;
  }

  const concealed = sortHandForDisplay(
    (player.concealed_tiles ?? []).filter(
      (tile) => !winningTile || tile.id !== winningTile.id,
    ),
    null,
    view.joker_code,
  );

  const seatLabel = winner.dealer ? "庄" : "子";
  const limit = winner.limit && winner.limit.length > 0 ? winner.limit : null;
  /* 四倍及以上役满的后端名是含糊的「多倍役满」，这里换成具体倍数。 */
  const limitLabel =
    limit && winner.yakuman_multiplier >= 4
      ? `${winner.yakuman_multiplier}倍役满`
      : limit;

  return (
    <div className="win-board">
      {/* ── character ── */}
      <div className="win-board__hero">
        {player.character_illustration_path && (
          <img src={player.character_illustration_path} alt="" />
        )}
        <div className="win-board__plate">
          <span className="win-board__plate-seat">{seatLabel}</span>
          <strong className="win-board__plate-name">{player.nickname}</strong>
        </div>
      </div>

      {/* ── hand ── */}
      <div className="win-board__hand">
        {concealed.map((tile) => (
          <WinTile key={tile.id} tile={tile} dora={doraIndicators} jokerCode={view.joker_code} />
        ))}
        {player.melds.map((meld) => (
          <span className="win-board__meld" key={meld.id}>
            {meld.tiles.map((tile) => (
              <WinTile key={tile.id} tile={tile} dora={doraIndicators} jokerCode={view.joker_code} />
            ))}
          </span>
        ))}
        {winningTile && (
          <span className="win-board__winning">
            <WinTile tile={winningTile} dora={doraIndicators} jokerCode={view.joker_code} />
          </span>
        )}
      </div>

      {/* ── indicators + reason ── */}
      <div className="win-board__indicators">
        {impact ? (
          <DoraStrip
            label="财神指示"
            tiles={view.joker_indicator ? [view.joker_indicator] : []}
            revealed={view.joker_indicator ? 1 : 0}
            slots={1}
          />
        ) : (
          <>
            <DoraStrip
              label="宝牌"
              tiles={doraIndicators}
              revealed={doraIndicators.length}
            />
            <DoraStrip
              label="里宝牌"
              tiles={uraIndicators}
              revealed={riichi ? uraIndicators.length : 0}
            />
          </>
        )}
        <span className="win-board__reason">{isTsumo ? "自摸" : "荣和"}</span>
      </div>

      {/* ── yaku ── 逐条亮出，JS 控制节奏，CSS 不再叠延迟 */}
      <div className="win-board__yaku">
        {winner.yaku.slice(0, yakuCount).map((yaku, yi) => (
          <div
            key={`${yaku.name}-${yi}`}
            className="win-yaku is-revealed"
            style={{ "--yaku-index": 0 } as CSSProperties}
          >
            <span className="win-yaku__name">{yaku.name}</span>
            {/* 全交只列那一条番种，本身不带点数，右边留空。 */}
            {impact && allIn ? null : (
              <span className="win-yaku__value">
                <b>{yaku.value}</b>
                {impact ? "点" : yaku.yakuman ? "倍" : "番"}
              </span>
            )}
          </div>
        ))}
      </div>

      {/* ── score ── 番种 / 番符亮完之后才出 */}
      {(showTally || showScore) && (
        <div
          className={`win-board__score${showScore ? " is-revealed" : ""}`}
          style={{ "--yaku-count": 0 } as CSSProperties}
        >
          {impact ? null : (
            <div className={`win-board__tally${showTally ? " is-revealed" : ""}`}>
              {winner.yakuman_multiplier > 0 ? (
                /* 有役满番种就只写「役满」，倍数统一进横幅大字。 */
                <b className="is-han">役满</b>
              ) : (
                <>
                  <b className="is-han">{winner.han}</b>
                  <i>番</i>
                  <b>{winner.fu}</b>
                  <i>符</i>
                </>
              )}
            </div>
          )}
          {showScore && (
            <div className="win-board__payout">
              {impact && allIn ? (
                <div className="win-board__limit">全交</div>
              ) : (
                <>
                  <div className="win-board__points">
                    <b>{winner.points.toLocaleString("en-US")}</b>
                    <i>点</i>
                  </div>
                  {limitLabel && <div className="win-board__limit">{limitLabel}</div>}
                </>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function DoraStrip({
  label,
  tiles,
  revealed,
  slots = 5,
}: {
  label: string;
  tiles: TileView[];
  revealed: number;
  /** 格子数。立直是五格宝牌，冲击麻将只有一张财神指示牌。 */
  slots?: number;
}) {
  return (
    <div className="dora-strip">
      <span className="dora-strip__label">{label}</span>
      <div className="dora-strip__tiles">
        {Array.from({ length: slots }, (_, i) => {
          const tile = i < revealed ? tiles[i] : undefined;
          return (
            <span key={i} className={`wtile wtile--dora${tile ? "" : " is-back"}`}>
              {tile && <img src={tileAssetPath(tile.code, "jp")} alt="" />}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function WinTile({ tile, dora, jokerCode }: { tile: TileView; dora: TileView[]; jokerCode?: string | null }) {
  const isJoker = isJokerTile(tile.code, jokerCode);
  return (
    <span className={`wtile${isDoraTile(tile.code, dora) ? " is-dora" : ""}${isJoker ? " is-joker" : ""}`}>
      <img src={tileAssetPath(tile.code, "jp")} alt="" />
    </span>
  );
}
