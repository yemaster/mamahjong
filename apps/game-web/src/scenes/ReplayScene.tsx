import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { LogOut } from "lucide-react";
import { apiFailure, gameApi } from "../api";
import { useSceneReady } from "../components/SceneTransition";
import { CallBannerLayer } from "../game/CallBanner";
import { DEFAULT_TABLECLOTH_ASSET, GameTable } from "../game/table";
import { HandSettlement } from "../game/HandSettlement";
import { MatchHud, type SeatWaitHint } from "../game/MatchHud";
import { MatchStage } from "../game/MatchStage";
import { PlayerHand2D } from "../game/PlayerHand2D";
import {
  loadTablePerspectiveSettings,
  tableCameraConfigFromSettings,
} from "../game/tableDisplaySettings";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import {
  DEFAULT_REPLAY_TOGGLES,
  ReplayControls,
  type ReplayToggles,
} from "../replay/ReplayControls";
import type { HandOption } from "../replay/HandPicker";
import { WallPanel } from "../replay/WallPanel";
import { buildSteps, handTitle, indexSteps } from "../replay/steps";
import { foldHand } from "../replay/replayState";
import { buildReplayView, handPointsBefore } from "../replay/replayView";
import { replayHandSettlement } from "../replay/replaySettlement";
import { useReplaySettlement } from "../replay/useReplaySettlement";
import { seatWaitInfo } from "../replay/waits";
import { matchRecordTitle, recordTitleParts } from "../replay/recordSummary";

/** 重演的牌桌不掷骰，中央固定一组点数就行。 */
const REPLAY_DICE: [number, number] = [3, 4];

/** 自动播放的节奏，一秒一步。 */
const AUTOPLAY_MS = 1000;

const noop = () => {};

/**
 * 牌谱重演。
 *
 * 界面就是正式对局那一套：`GameTable` 画桌子、`PlayerHand2D` 画自己那手、
 * `MatchHud` 画头像和宝牌，喂进去的是从事件日志折叠出来的合成 `MatchView`
 * （见 `docs/match-record-replay.md` 第五节）。这一页只多两样东西：底部的控制条
 * 和一块牌山面板。
 */
export default function ReplayScene({ matchId }: { matchId: string }) {
  const token = useAuthStore((state) => state.token);
  const userId = useAuthStore((state) => state.identity?.id);
  const selectedTableclothId = useAuthStore(
    (state) => state.identity?.profile.selected_tablecloth_id,
  );
  const record = useQuery({
    queryKey: ["match-record", matchId],
    queryFn: () => gameApi.matchRecord(matchId, token!),
    enabled: Boolean(token),
  });
  const tablecloths = useQuery({
    queryKey: ["tablecloths"],
    queryFn: gameApi.tablecloths,
  });
  useSceneReady(!record.isLoading);

  const tableclothPath =
    tablecloths.data?.tablecloths.find(
      (tablecloth) => tablecloth.id === selectedTableclothId,
    )?.texture_path ??
    tablecloths.data?.tablecloths.find((tablecloth) => tablecloth.is_default)
      ?.texture_path ??
    DEFAULT_TABLECLOTH_ASSET;
  const cameraConfig = useMemo(
    () => tableCameraConfigFromSettings(loadTablePerspectiveSettings(userId)),
    [userId],
  );

  const [stepIndex, setStepIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [wallOpen, setWallOpen] = useState(false);
  const [barCollapsed, setBarCollapsed] = useState(false);
  const [toggles, setToggles] = useState<ReplayToggles>(
    DEFAULT_REPLAY_TOGGLES,
  );

  const data = record.data;
  const steps = useMemo(() => (data ? buildSteps(data) : []), [data]);
  const handIndex = useMemo(() => indexSteps(steps), [steps]);

  /* 换了一份牌谱就从头开始，别停在上一份的步数上。 */
  useEffect(() => {
    setStepIndex(0);
    setPlaying(false);
  }, [matchId]);

  /* 观战座位默认是自己坐过的那个位置；找不到就从东家看。 */
  const observerSeat = useMemo(() => {
    const seat = data?.players.find((player) => player.user_id === userId)?.seat;
    return seat ?? 0;
  }, [data, userId]);

  const step = steps[Math.min(stepIndex, steps.length - 1)];
  const currentHand = step ? data?.hands[step.handIndex] : undefined;
  const seatCount = data?.players.length ?? 4;

  const currentHandSteps = step
    ? handIndex.find((entry) => entry.handIndex === step.handIndex)
    : undefined;
  const handEnd = currentHandSteps
    ? currentHandSteps.start + currentHandSteps.count - 1
    : 0;

  const state = useMemo(() => {
    if (!currentHand || !step) return null;
    return foldHand(currentHand, seatCount, step.eventIndex);
  }, [currentHand, seatCount, step]);

  /*
   * 结算只在本局最后一步给：和牌那一下走完了，牌桌才进结算相位。
   * 流局不给——用户要看的是胡牌那一屏，流局既不摊听牌也不开面板。
   */
  const settlement = useMemo(() => {
    if (!currentHand || stepIndex < handEnd) return null;
    return replayHandSettlement(currentHand);
  }, [currentHand, stepIndex, handEnd]);

  const view = useMemo(() => {
    if (!data || !state || !step) return null;
    return buildReplayView({
      record: data,
      state,
      handIndex: step.handIndex,
      observerSeat,
      /* 每一步（以及换视角）都得换个数，牌桌的重绘 effect 盯的就是它。 */
      version: stepIndex * seatCount + observerSeat + 1,
      pointsBefore: handPointsBefore(data, step.handIndex),
      settlement: settlement?.view ?? null,
    });
  }, [data, state, step, observerSeat, stepIndex, seatCount, settlement]);

  /* 和牌动画：喊声、砸牌、摊手，最后升面板。 */
  const timeline = useReplaySettlement(
    view,
    `${step?.handIndex ?? -1}:${stepIndex}`,
  );

  const waitInfo = useMemo(() => (view ? seatWaitInfo(view) : new Map()), [view]);
  const dangerTileCodes = useMemo(() => {
    if (!toggles.dangerTiles) return [];
    const codes = new Set<string>();
    for (const info of waitInfo.values()) {
      for (const code of info.waits) codes.add(code);
    }
    return [...codes];
  }, [toggles.dangerTiles, waitInfo]);
  const waitHints = useMemo(() => {
    if (!toggles.tenpaiHints) return undefined;
    const hints = new Map<number, SeatWaitHint>();
    for (const [seat, info] of waitInfo) {
      hints.set(seat, { waits: info.waits, remaining: info.remaining });
    }
    return hints;
  }, [toggles.tenpaiHints, waitInfo]);

  /* 跳局下拉里每一局都要写四家的点数变化，昵称按座次配一遍。 */
  const handOptions = useMemo<HandOption[]>(() => {
    if (!data) return [];
    const nicknames = new Map(
      data.players.map((player) => [player.seat, player.nickname]),
    );
    return data.hands.map((hand) => ({
      handIndex: hand.hand_index,
      title: handTitle(hand),
      deltas: hand.point_deltas.map((delta, seat) => ({
        seat,
        nickname: nicknames.get(seat) ?? `${seat}`,
        delta,
      })),
    }));
  }, [data]);

  /* 自动播放播到本局末尾就停，不会自己翻到下一局。 */
  useEffect(() => {
    if (!playing) return;
    if (stepIndex >= handEnd) {
      setPlaying(false);
      return;
    }
    const timer = window.setTimeout(
      () => setStepIndex((index) => index + 1),
      AUTOPLAY_MS,
    );
    return () => window.clearTimeout(timer);
  }, [playing, stepIndex, handEnd]);

  if (
    record.isLoading ||
    !view ||
    !step ||
    !currentHand ||
    !currentHandSteps ||
    !data
  ) {
    return (
      <section className="replay-screen replay-screen--plain">
        <p className="replay-screen__hint">
          {record.error ? apiFailure(record.error).message : "载入牌谱…"}
        </p>
        <button type="button" onClick={() => navigateTo({ kind: "records" })}>
          返回牌谱
        </button>
      </section>
    );
  }

  const selectHand = (next: number) => {
    const target = handIndex.find((entry) => entry.handIndex === next);
    if (!target) return;
    setPlaying(false);
    setStepIndex(target.start);
  };

  const selectTurn = (turn: number) => {
    const start = currentHandSteps.turnStarts[turn - 1];
    if (start === undefined) return;
    setPlaying(false);
    setStepIndex(start);
  };

  const stepForward = () => {
    setPlaying(false);
    setStepIndex((index) => Math.min(steps.length - 1, index + 1));
  };

  /*
   * 点空白处就是下一步。牌桌那层（`.match-stage-root`）本身 `pointer-events: none`，
   * 点在桌面上的事件会一路冒到这里来，所以只要把「点在控件上」的那几种滤掉就行。
   */
  const onScreenClick = (event: React.MouseEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement | null;
    if (
      target?.closest(
        ".replay-bar, .replay-picker__panel, .replay-wall, .match-utility, .win-screen, .match-settlement",
      )
    ) {
      return;
    }
    stepForward();
  };

  return (
    <div className="match-screen replay-screen" onClick={onScreenClick}>
      {/*
       * `dimTsumogiri` 和 `instantDraw` 都是牌谱独有的：手切摸切是复盘才该看到的
       * 情报，对局里不给；摸牌不飞是因为一步就是一个状态，下一步一到桌子推倒重来，
       * 飞到半路的牌会跟着没掉。
       */}
      <GameTable
        view={view}
        openingPhase="play"
        dice={REPLAY_DICE}
        onTileDiscard={noop}
        cameraConfig={cameraConfig}
        tableclothPath={tableclothPath}
        dangerTileCodes={dangerTileCodes}
        revealAllHands={toggles.revealHands}
        dimTsumogiri
        instantDraw
        settlementRevealSeats={timeline.revealSeats}
        settlementWinningTileSeats={timeline.winningTileSeats}
      />
      <div className="match-utility" aria-label="牌谱功能">
        <button
          type="button"
          onClick={() => navigateTo({ kind: "records" })}
          aria-label="返回牌谱列表"
          title="返回牌谱列表"
        >
          <LogOut aria-hidden="true" />
        </button>
      </div>

      <MatchStage>
        <PlayerHand2D
          view={view}
          openingPhase="play"
          onTileDiscard={noop}
          riichiSelecting={false}
          instantDraw
        />
        <MatchHud view={view} seatWaitHints={waitHints} />

        <div className="replay-title">
          {matchRecordTitle(recordTitleParts(data))}
          <span>{handTitle(currentHand)}</span>
        </div>

        {wallOpen && (
          <WallPanel
            wall={currentHand.wall}
            drawnBy={state?.drawnBy ?? new Map()}
            observerSeat={observerSeat}
            seatCount={seatCount}
            onClose={() => setWallOpen(false)}
          />
        )}

        <ReplayControls
          wallOpen={wallOpen}
          onToggleWall={() => setWallOpen((open) => !open)}
          handOptions={handOptions}
          handIndex={step.handIndex}
          onSelectHand={selectHand}
          turnCount={currentHandSteps.turnStarts.length}
          turnIndex={step.turnIndex}
          onSelectTurn={selectTurn}
          canStepBack={stepIndex > 0}
          canStepForward={stepIndex < steps.length - 1}
          onStepBack={() => {
            setPlaying(false);
            setStepIndex((index) => Math.max(0, index - 1));
          }}
          onStepForward={stepForward}
          playing={playing}
          onTogglePlay={() => setPlaying((value) => !value)}
          collapsed={barCollapsed}
          onToggleCollapsed={() => setBarCollapsed((value) => !value)}
          toggles={toggles}
          onTogglesChange={setToggles}
        />

        {/*
         * 面板要番符役种才写得出来，旧牌谱没存那一段（见 `replayHandSettlement`）：
         * 那几局演出照播，最后这块不升起来。
         */}
        <HandSettlement
          view={view}
          showPanel={timeline.panelVisible && settlement?.detailed === true}
          confirmReady
          secondsRemaining={0}
          locallyConfirmed={false}
          onConfirm={timeline.dismissPanel}
          confirmLabel="收起"
        />
        <CallBannerLayer view={view} banners={timeline.banners} />
      </MatchStage>
    </div>
  );
}
