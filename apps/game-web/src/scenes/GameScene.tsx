import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { CircleHelp, LogOut, Settings } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError, gameApi, apiFailure } from "../api";
import { Modal } from "../components/Modal";
import {
  useSceneReady,
} from "../components/SceneTransition";
import {
  playRiichiMusic,
  preloadMusic,
  resolveTrack,
  stopRiichiMusic,
} from "../audio/music";
import {
  DISCARD_SFX,
  FU_APPEAR_SFX,
  HULE_FAN_OUT_SFX,
  MOUSECLICK_SFX,
  NEWROUND_PAIS_SFX,
  SCORE_APPEAR_SFX,
  SCORE_CHANGE_SFX,
  playSfx,
  preloadSfx,
} from "../audio/sfx";
import {
  actionVoices,
  playVoice,
  preloadVoices,
  resolveVoice,
} from "../audio/voice";
import {
  canLocalPlayerDiscard,
  DEFAULT_TABLECLOTH_ASSET,
  GameTable,
  type GameTableHandle,
  settlementCoveringSeats,
  TSUMO_THROW_MS,
} from "../game/table";
import { CallBannerLayer } from "../game/CallBanner";
import { MatchStage } from "../game/MatchStage";
import {
  detectMeldCalls,
  detectRiichiCalls,
  drawReasonLabel,
  drawRevealOrder,
  isDoubleRiichiTurn,
  type CallBannerItem,
  type CallKind,
} from "../game/callBanners";
import {
  CALL_BANNER_MS,
  POINTS_REVEAL_MS,
  SETTLEMENT_COUNTDOWN_MS,
  SETTLEMENT_REVEAL_BUDGET_MS,
} from "../game/animationTiming";
import { ActionPanel } from "../game/ActionPanel";
import { SettingsPanel } from "../game/SettingsPanel";
import { ChatBox, ChatMessages } from "../game/ChatBox";
import { ChiOptionPicker } from "../game/ChiOptionPicker";
import { observerChiOptions } from "../game/chiOptions";
import { commandRejectionText } from "../game/commandErrors";
import { ExitVotePanel } from "../game/ExitVotePanel";
import { MatchHud } from "../game/MatchHud";
import { HandSettlement } from "../game/HandSettlement";
import { KanPointOverlay } from "../game/KanPointOverlay";
import { PointChangeOverlay } from "../game/PointChangeOverlay";
import { MatchAssistControls } from "../game/MatchAssistControls";
import { PlayerHand2D } from "../game/PlayerHand2D";
import { applyViewPatch } from "../game/viewPatch";
import { YakuReferenceModal } from "../game/YakuReference";
import {
  automaticMatchCommand,
  loadMatchAssistSettings,
  resetPerHandMatchAssistSettings,
  saveMatchAssistSettings,
} from "../game/matchAssist";
import {
  loadTablePerspectiveSettings,
  tableCameraConfigFromSettings,
} from "../game/tableDisplaySettings";
import {
  openingDice,
} from "../game/OpeningSequence";
import { useMatchOpening } from "../game/useMatchOpening";
import { MatchStream } from "../ws";
import { useAuthStore } from "../stores/authStore";
import { useGameStore } from "../stores/gameStore";
import { navigateTo } from "../routing";
import type {
  GameCommandName,
  KanPointsView,
  MatchView,
  VoiceKind,
  WaitingTileView,
} from "../types";

const POLL_MS = 500;
/** 对局音乐最多load这么久，再慢也不能把整桌拖到判超时。 */
const MATCH_MUSIC_LOAD_TIMEOUT_MS = 40_000;
/**
 * 四家的操作语音最多load这么久。
 *
 * 比音乐那道短：语音是几十 KB 的小文件，真下不来也不该让整桌陪着等——缺了
 * 顶多是这一局没声音，不能因此把对局拖成超时作废。
 */
const MATCH_VOICE_LOAD_TIMEOUT_MS = 15_000;

interface GameSceneProps {
  matchId: string;
}

export default function GameScene({ matchId }: GameSceneProps) {
  const token = useAuthStore((s) => s.token);
  const userId = useAuthStore((s) => s.identity?.id);
  const selectedTableclothId = useAuthStore(
    (s) => s.identity?.profile.selected_tablecloth_id,
  );
  const tablecloths = useQuery({
    queryKey: ["tablecloths"],
    queryFn: gameApi.tablecloths,
  });
  const tableclothPath =
    tablecloths.data?.tablecloths.find(
      (tablecloth) => tablecloth.id === selectedTableclothId,
    )?.texture_path ??
    tablecloths.data?.tablecloths.find((tablecloth) => tablecloth.is_default)
      ?.texture_path ??
    DEFAULT_TABLECLOTH_ASSET;
  const selectedMatchMusicId = useAuthStore(
    (s) => s.identity?.profile.selected_match_music_id,
  );
  const musicCatalog = useQuery({
    queryKey: ["music-tracks"],
    queryFn: () => gameApi.musicTracks(),
    staleTime: 5 * 60_000,
  });
  const matchMusicPath =
    resolveTrack(
      musicCatalog.data?.music_tracks,
      "match",
      selectedMatchMusicId,
    )?.audio_path ?? null;
  /* 开局要把四家角色的语音都load下来，不只是自己那位。 */
  const characterCatalog = useQuery({
    queryKey: ["characters"],
    queryFn: gameApi.characters,
    staleTime: 5 * 60_000,
  });
  const charactersById = useMemo(
    () =>
      new Map(
        (characterCatalog.data?.characters ?? []).map((character) => [
          character.id,
          character,
        ]),
      ),
    [characterCatalog.data],
  );
  const tableCameraConfig = useMemo(
    () =>
      tableCameraConfigFromSettings(
        loadTablePerspectiveSettings(userId),
      ),
    [userId],
  );
  /* 只订阅主场景真正需要的状态；时钟与在线帧不能让整页 GameScene 重绘。 */
  const matchView = useGameStore((state) => state.matchView);
  const wsState = useGameStore((state) => state.wsState);
  const setMatchView = useGameStore((state) => state.setMatchView);
  const updateClocks = useGameStore((state) => state.updateClocks);
  const updatePresence = useGameStore((state) => state.updatePresence);
  const setWsState = useGameStore((state) => state.setWsState);
  const reset = useGameStore((state) => state.reset);
  const streamRef = useRef<MatchStream | null>(null);
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  /* 服务端拒掉一条指令时的临时提示，几秒后自己消失。 */
  const [notice, setNotice] = useState<string | null>(null);
  const noticeTimer = useRef<number | null>(null);
  /* 本家的对局素材load完了没有。全场load完之前后端一条命令都不收。 */
  const [assetsLoaded, setAssetsLoaded] = useState(false);
  const assetsRequested = useRef(false);
  const [riichiSelecting, setRiichiSelecting] = useState(false);
  /*
   * 正在挑吃哪一组。存的是打开那一刻的 `version` 而不是一个开关：这一巡一过
   * （别人先鸣了、或者自己按了取消）视图就换了个号，选择状态自然作废，用不着
   * 再找地方把开关关回去——关漏一处，下一次能吃时框会自己弹出来。
   */
  const [chiSelectingVersion, setChiSelectingVersion] = useState<number | null>(
    null,
  );
  const [assistSettings, setAssistSettings] = useState(() =>
    loadMatchAssistSettings(userId),
  );
  const assistSettingsHandKey = useRef<string | null>(null);
  const [yakuReferenceOpen, setYakuReferenceOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settlementRevealSeats, setSettlementRevealSeats] = useState<number[]>([]);
  const [settlementWinningTileSeats, setSettlementWinningTileSeats] = useState<
    number[]
  >([]);
  const gameTableRef = useRef<GameTableHandle>(null);
  const focusTableTile = useCallback((code: string | null) => {
    gameTableRef.current?.setFocusedTileCode(code);
  }, []);
  const [callBanners, setCallBanners] = useState<CallBannerItem[]>([]);
  const [settlementPanelVisible, setSettlementPanelVisible] = useState(false);
  const [settlementConfirmReady, setSettlementConfirmReady] = useState(false);
  const [settlementSeconds, setSettlementSeconds] = useState(
    SETTLEMENT_COUNTDOWN_MS / 1000,
  );
  const [settlementLocallyConfirmed, setSettlementLocallyConfirmed] =
    useState(false);
  const [settlementPointsPhase, setSettlementPointsPhase] = useState(false);
  const [pointsConfirmed, setPointsConfirmed] = useState(false);
  const settlementPlayedSent = useRef(false);
  const settlementConfirmSent = useRef(false);
  const pointsPhaseStarted = useRef(false);
  const settlementPointTimers = useRef<number[]>([]);
  const bannerTimers = useRef<number[]>([]);
  const bannerSequence = useRef(0);
  const callDiffView = useRef<MatchView | null>(null);
  const discardDiffView = useRef<MatchView | null>(null);
  const automaticCommandKey = useRef<string | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const matchUnavailable = useRef(false);

  useEffect(() => {
    saveMatchAssistSettings(userId, assistSettings);
  }, [assistSettings, userId]);

  /* 每一局只重置一次。视图版本会不停增长，不能拿 version 当局号，否则玩家刚打开
     的按钮会在下一帧又被关掉。断线后重新进入当前局也按一次新局处理。 */
  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    const handKey = `${matchView.id}:${matchView.hand_index}`;
    if (assistSettingsHandKey.current === handKey) return;
    assistSettingsHandKey.current = handKey;
    setAssistSettings(resetPerHandMatchAssistSettings);
  }, [matchId, matchView?.hand_index, matchView?.id]);

  /* 指令被拒时把服务端给的原因摆到桌上，几秒后收走。 */
  const showNotice = useCallback((message: string) => {
    setNotice(message);
    if (noticeTimer.current) window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(null), 4000);
  }, []);

  useEffect(
    () => () => {
      if (noticeTimer.current) window.clearTimeout(noticeTimer.current);
    },
    [],
  );

  /* ── Initial HTTP fetch ──────────────── */
  /*
    只在首屏和掉线轮询时走这一路。HTTP 响应会和连接上的补丁赛跑，拉回来的可能
    比连接上已经推进到的更旧，所以只有比手上这份新才允许覆盖。
  */
  const fetchView = useCallback(async () => {
    if (!token || matchUnavailable.current) return;
    try {
      const view = await gameApi.matchView(matchId, token);
      const held = useGameStore.getState().matchView;
      if (!held || view.version >= held.version) {
        setMatchView(view);
      }
      setError(null);
    } catch (err: unknown) {
      if (err instanceof ApiError && err.status === 404) {
        matchUnavailable.current = true;
        setError("对局不存在，正在返回大厅…");
        navigateTo({ kind: "lobby" });
        return;
      }
      setError(apiFailure(err).message);
    }
  }, [token, matchId, setMatchView]);

  /* ── Mount / unmount ─────────────────── */
  useEffect(() => {
    matchUnavailable.current = false;
    reset();
    fetchView();

    /* Start WebSocket. */
    if (token) {
      const host = window.location.host;
      const baseUrl = `${window.location.protocol}//${host}`;
      const stream = new MatchStream(
        baseUrl,
        token,
        matchId,
        matchView?.event_sequence ?? 0,
        {
          onEvent: (event) => {
            switch (event.kind) {
              case "view_snapshot": {
                /* 快照是自足的一份真相，收到就整份换掉。 */
                const view = event.view as MatchView;
                setMatchView(view);
                streamRef.current?.noteCursor(view.event_sequence);
                setError(null);
                break;
              }
              case "view_patch": {
                const held = useGameStore.getState().matchView;
                if (!held || held.version !== event.baseVersion) {
                  /*
                    补丁是从服务端记着的那一份算出来的，底子对不上就绝不能打，
                    宁可请求重来，也不能把界面推到一个两边都不认识的状态。
                  */
                  streamRef.current?.requestResync();
                  break;
                }
                const view = applyViewPatch(held, event.ops) as MatchView;
                setMatchView(view);
                streamRef.current?.noteCursor(view.event_sequence);
                break;
              }
              case "events_arrived":
                /* 视图订阅收不到事件帧；老服务端才会走到这里。 */
                fetchView();
                break;
              case "clock":
                updateClocks(event.seats);
                if (
                  event.version > 0 &&
                  event.version !==
                    useGameStore.getState().matchView?.version
                ) {
                  streamRef.current?.requestResync();
                }
                break;
              case "presence":
                updatePresence(event.seats);
                break;
              case "disconnected":
                /* Polling fallback starts below. */
                break;
              case "reconnected":
                /* 新连接没有上一份视图，服务端紧接着就补一份快照。 */
                break;
              case "command_rejected":
                /* 多半是本地版本落后了，顺手要一份最新的回来。 */
                showNotice(commandRejectionText(event.code));
                streamRef.current?.requestResync();
                break;
            }
          },
          onStateChange: (state) =>
            setWsState(
              state === "connected"
                ? "connected"
                : state === "connecting"
                  ? "connecting"
                  : "disconnected",
            ),
        },
      );
      stream.connect();
      streamRef.current = stream;
    }

    /* HTTP polling fallback while disconnected. */
    pollTimer.current = setInterval(() => {
      const s = useGameStore.getState();
      if (s.wsState === "disconnected") {
        fetchView();
      }
    }, POLL_MS);

    return () => {
      streamRef.current?.disconnect();
      if (pollTimer.current) clearInterval(pollTimer.current);
      /* 离开这局时清空全局 store，下一局 mount 时不会看到上一局的残影——
         结果页、投票退出等重定向判定就不会被旧数据触发。 */
      reset();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [matchId, token]);

  /* ── Commands ─────────────────────────── */
  const onCommand = useCallback(
    (name: GameCommandName, payload?: unknown) => {
      if (!token) return;
      const view = useGameStore.getState().matchView;
      if (!view) return;

      if (wsState === "connected" && streamRef.current) {
        streamRef.current.sendCommand(name, payload, view.version);
        return;
      }

      gameApi
        .gameCommand(matchId, view.version, name, payload, token)
        .then((v) => setMatchView(v))
        .catch((err: unknown) => {
          if (err instanceof ApiError && err.status === 404) {
            matchUnavailable.current = true;
            setError("对局不存在，正在返回大厅…");
            navigateTo({ kind: "lobby" });
            return;
          }
          setError(apiFailure(err).message);
        });
    },
    [token, matchId, setMatchView, wsState],
  );
  const {
    phase: openingPhase,
    allAssetsReady,
    assetsTimedOut,
  } = useMatchOpening({
    matchId,
    view: matchView,
    assetsLoaded,
    onCommand,
  });

  /* 座位 → 这一家角色该喊的那条语音。
     查表塞在 ref 里，`shout` 才能保持稳定的身份：它被结算那串定时器用着，
     身份一变整串就得重排，声音会重放一遍。 */
  const seatVoice = useRef<(seat: number, kind: VoiceKind) => string | null>(
    () => null,
  );
  useEffect(() => {
    seatVoice.current = (seat, kind) => {
      const player = matchView?.players.find(
        (candidate) => candidate.seat === seat,
      );
      if (!player?.character_id) return null;
      return resolveVoice(charactersById.get(player.character_id), kind);
    };
  }, [charactersById, matchView]);

  const voiceTimers = useRef<number[]>([]);
  /* 让某一家喊一声。`delay` 跟着对应横幅走，字和声音一起出来。 */
  const shout = useCallback(
    (seat: number, kind: VoiceKind, delay = 0) => {
      const play = () => playVoice(seatVoice.current(seat, kind));
      if (delay <= 0) {
        play();
        return;
      }
      voiceTimers.current.push(window.setTimeout(play, delay));
    },
    [],
  );

  useEffect(
    () => () => {
      voiceTimers.current.forEach(window.clearTimeout);
      voiceTimers.current = [];
    },
    [],
  );

  /* Pops a 吃/碰/杠/立直/和了 banner beside the seat that acted. */
  const pushBanner = useCallback(
    (
      kind: CallKind,
      seat: number | null,
      delay = 0,
      /* 吃碰杠立直的横幅时长后端也要知道（见 animationTiming）。 */
      lifetime = CALL_BANNER_MS,
      waits?: WaitingTileView[],
      /** 挂到点数动画开始为止的横幅（流局的听牌/不听）。 */
      hold = false,
      /** 覆盖默认文字，流局用来写明具体的流局原因。 */
      label?: string,
    ) => {
      bannerSequence.current += 1;
      const id = `${kind}-${seat ?? "table"}-${bannerSequence.current}`;
      bannerTimers.current.push(
        window.setTimeout(() => {
          setCallBanners((current) => [
            ...current,
            { id, kind, seat, label, waits, holdMs: hold ? lifetime : undefined },
          ]);
          bannerTimers.current.push(
            window.setTimeout(() => {
              setCallBanners((current) =>
                current.filter((banner) => banner.id !== id),
              );
            }, lifetime),
          );
        }, delay),
      );
    },
    [],
  );

  useEffect(
    () => () => {
      bannerTimers.current.forEach(window.clearTimeout);
      bannerTimers.current = [];
    },
    [],
  );

  /* Watches the view for new melds and riichi declarations. */
  useEffect(() => {
    if (!matchView) return;
    const previous = callDiffView.current;
    callDiffView.current = matchView;
    if (
      !previous ||
      previous.id !== matchView.id ||
      previous.hand_index !== matchView.hand_index
    ) {
      return;
    }
    const double = isDoubleRiichiTurn(matchView);
    for (const seat of detectRiichiCalls(matchView, previous)) {
      pushBanner("riichi", seat);
      /* 横幅上两立直和立直写的是同一个字，喊出来的却是两条不同的语音。 */
      shout(seat, double ? "double_riichi" : "riichi");
      /* 把背景音乐换成这一家的立直曲目。 */
      const riichiPath = matchView.players.find(
        (player) => player.seat === seat,
      )?.riichi_music_path;
      if (riichiPath) {
        playRiichiMusic(riichiPath);
      }
    }
    for (const call of detectMeldCalls(matchView, previous)) {
      pushBanner(call.kind, call.seat);
      shout(call.seat, call.kind);
    }
  }, [matchView, pushBanner, shout]);

  /* 有玩家打出新牌时，等牌飞到牌河之后播落地音效。 */
  useEffect(() => {
    if (!matchView) return;
    const previous = discardDiffView.current;
    discardDiffView.current = matchView;
    if (
      !previous ||
      previous.id !== matchView.id ||
      previous.hand_index !== matchView.hand_index
    ) {
      return;
    }
    if (openingPhase !== "play") return;
    const hasNewDiscard = matchView.players.some((player) => {
      const prev = previous.players.find((p) => p.seat === player.seat);
      return player.discards.length > (prev?.discards.length ?? player.discards.length);
    });
    if (hasNewDiscard) {
      window.setTimeout(() => playSfx(DISCARD_SFX), 300);
    }
  }, [matchView, openingPhase]);

  /* 本地结算动画播完了，报告服务端一声。幂等。
     全场都报告（或宽限到期）之后服务端才开确认窗口并起算那五秒，所以各家的
     按钮和倒计时是同一个数，不会有人先点到、有人还在播。 */
  const sendSettlementPlayed = useCallback(() => {
    const view = useGameStore.getState().matchView;
    if (!view?.hand_settlement || settlementPlayedSent.current) return;
    settlementPlayedSent.current = true;
    onCommand("game.settlement_played", {
      hand_index: view.hand_index,
    });
  }, [onCommand]);

  /* 点确认。窗口没开时按钮根本不出现，服务端也会拒绝。 */
  const sendSettlementConfirm = useCallback(() => {
    const view = useGameStore.getState().matchView;
    if (!view?.hand_settlement || settlementConfirmSent.current) return;
    settlementConfirmSent.current = true;
    setPointsConfirmed(true);
    onCommand("game.confirm_settlement", {
      hand_index: view.hand_index,
    });
  }, [onCommand]);

  /* Starts the point animation, the last thing the client plays on its own.
     点棒滚完就报告播完，之后等服务端下发确认窗口。 */
  const startPointsPhase = useCallback(() => {
    const view = useGameStore.getState().matchView;
    if (!view?.hand_settlement || pointsPhaseStarted.current) {
      return;
    }
    pointsPhaseStarted.current = true;
    setSettlementLocallyConfirmed(true);
    setSettlementConfirmReady(false);
    setSettlementPointsPhase(true);
    setPointsConfirmed(false);
    settlementPointTimers.current.push(
      window.setTimeout(() => sendSettlementPlayed(), POINTS_REVEAL_MS),
    );
  }, [sendSettlementPlayed]);

  const settlementKey = matchView?.hand_settlement
    ? `${matchView.id}:${matchView.hand_index}:${matchView.hand_settlement.reason}`
    : null;

  /* Clears the timers that drive the point-change phase. */
  const resetPointsPhase = useCallback(() => {
    settlementPointTimers.current.forEach(window.clearTimeout);
    settlementPointTimers.current = [];
    pointsPhaseStarted.current = false;
    setSettlementPointsPhase(false);
    setPointsConfirmed(false);
  }, []);

  /* 一局结算或新一局开始时，立直音乐就该停了，恢复原来的对局曲。 */
  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    if (matchView.hand_settlement || matchView.result) {
      stopRiichiMusic();
    }
  }, [
    matchId,
    matchView?.hand_settlement,
    matchView?.result,
    matchView?.id,
  ]);

  /* 每局开头也恢复对局曲，免得上一局的立直曲留到新局。 */
  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    stopRiichiMusic();
  }, [matchId, matchView?.hand_index, matchView?.id]);

  useEffect(() => {
    if (!settlementKey || !matchView?.hand_settlement) {
      setSettlementRevealSeats([]);
      setSettlementWinningTileSeats([]);
      setSettlementPanelVisible(false);
      setSettlementConfirmReady(false);
      setSettlementSeconds(SETTLEMENT_COUNTDOWN_MS / 1000);
      setSettlementLocallyConfirmed(false);
      resetPointsPhase();
      settlementPlayedSent.current = false;
      settlementConfirmSent.current = false;
      return;
    }

    settlementPlayedSent.current = matchView.hand_settlement.played_seats.includes(
      matchView.observer_seat,
    );
    settlementConfirmSent.current = matchView.hand_settlement.confirmed_seats.includes(
      matchView.observer_seat,
    );
    setSettlementRevealSeats([]);
    setSettlementWinningTileSeats([]);
    setSettlementPanelVisible(false);
    setSettlementConfirmReady(false);
    setSettlementSeconds(SETTLEMENT_COUNTDOWN_MS / 1000);
    setSettlementLocallyConfirmed(false);
    resetPointsPhase();

    const settlement = matchView.hand_settlement;
    const winnerSeats = settlement.winners.map((w) => w.seat);
    const isDraw = winnerSeats.length === 0;
    const isTsumo = settlement.reason === "tsumo";
    const timers: number[] = [];
    const revealSeat = (seat: number) =>
      setSettlementRevealSeats((current) =>
        current.includes(seat) ? current : [...current, seat],
      );

    if (isDraw) {
      /* 流局 opens the hands from the dealer round, half a second apart: tenpai
         hands fall open with their waits, the rest turn face down. Everything
         stays on screen for a beat, then straight to the point animation. */
      /* 横幅上直接写明是怎么流的：荒牌流局，还是四种途中流局之一。 */
      pushBanner(
        "draw",
        null,
        0,
        1500,
        undefined,
        false,
        drawReasonLabel(settlement.reason),
      );
      const order = drawRevealOrder(matchView);
      const firstAt = 900;
      const stepMs = 500;
      /* 四家摊完之后留一拍，再进点数动画。摊牌阶段
         有硬上界，服务端的兜底就是照这个上界算出来的。 */
      const pointsAt = Math.min(
        firstAt + (order.length - 1) * stepMs + 1800,
        SETTLEMENT_REVEAL_BUDGET_MS,
      );
      const isImpactDraw = matchView.variant_kind === "impact";
      order.forEach((seat, index) => {
        const at = firstAt + index * stepMs;
        timers.push(window.setTimeout(() => revealSeat(seat), at));
        /* 冲击麻将荒牌流局不区分听/不听，直接摊牌进点数。 */
        if (!isImpactDraw) {
          const tenpai = settlement.tenpai_seats.includes(seat);
          /* 听牌者旁边直接摆出听的牌，不听的就挂个「不听」；两者都留到
             点数动画开始才收走，方便一眼看完全场。 */
          const waits = tenpai
            ? (matchView.players.find((player) => player.seat === seat)
                ?.waiting_tiles ?? [])
            : undefined;
          const shownAt = at + 160;
          pushBanner(
            tenpai ? "tenpai" : "noten",
            seat,
            shownAt,
            Math.max(600, pointsAt - shownAt),
            waits,
            true,
          );
        }
      });
      timers.push(window.setTimeout(() => startPointsPhase(), pointsAt));
      return () => {
        timers.forEach(window.clearTimeout);
        settlementPointTimers.current.forEach(window.clearTimeout);
        settlementPointTimers.current = [];
      };
    }

    // 自摸 / 荣和: shout the call beside the winner, lay the hand out, then
    // turn over whoever has to cover.
    winnerSeats.forEach((seat, index) => {
      pushBanner(isTsumo ? "tsumo" : "ron", seat, index * 240, 1600);
      shout(seat, isTsumo ? "tsumo" : "ron", index * 240);
    });

    let handRevealAt = 300;
    if (isTsumo) {
      /* 自摸先把那张牌从高处砸到桌上，等那一下的灰扬起来，手牌才跟着瘫下去。 */
      winnerSeats.forEach((seat, index) => {
        timers.push(
          window.setTimeout(() => {
            setSettlementWinningTileSeats((current) =>
              current.includes(seat) ? current : [...current, seat],
            );
          }, handRevealAt + index * 240),
        );
      });
      handRevealAt += TSUMO_THROW_MS + 200;
    }

    winnerSeats.forEach((seat, index) => {
      timers.push(
        window.setTimeout(() => revealSeat(seat), handRevealAt + index * 320),
      );
    });
    const winnerRevealEnd = handRevealAt + winnerSeats.length * 320;

    // On 荣和 only the player who dealt in turns their tiles over, on 自摸
    // everyone else does.
    const coveringSeats = settlementCoveringSeats(matchView);
    if (coveringSeats.length > 0) {
      timers.push(
        window.setTimeout(() => {
          setSettlementRevealSeats((current) => {
            const next = [...current];
            for (const seat of coveringSeats) {
              if (!next.includes(seat)) next.push(seat);
            }
            return next;
          });
        }, winnerRevealEnd + 220),
      );
    }

    const panelAt = winnerRevealEnd + 220 + 340 + 900;
    const longestYakuCount = Math.max(
      0,
      ...matchView.hand_settlement.winners.map(
        (winner) => winner.yaku.length,
      ),
    );
    const yakuRevealDuration =
      longestYakuCount === 0 ? 0 : 800 + longestYakuCount * 520 + 700;
    /* 确认按钮不等役种一条条翻完：面板一亮就给出来，想快的人可以立刻点掉。
       自动推进的读秒还是等动画播完才开始，愿意看的人一条也不会少。役种特别
       多时截到摊牌上界为止，服务端的兜底就是照这个上界算出来的。 */
    const confirmAt = panelAt + 360;
    const countdownAt = Math.min(
      panelAt + Math.max(1200, yakuRevealDuration),
      SETTLEMENT_REVEAL_BUDGET_MS,
    );
    const settlementCountdown = SETTLEMENT_COUNTDOWN_MS / 1000;
    timers.push(
      window.setTimeout(() => setSettlementPanelVisible(true), panelAt),
    );
    timers.push(
      window.setTimeout(() => setSettlementConfirmReady(true), confirmAt),
    );
    for (let second = 1; second <= settlementCountdown; second += 1) {
      timers.push(
        window.setTimeout(
          () => setSettlementSeconds(Math.max(0, settlementCountdown - second)),
          countdownAt + second * 1000,
        ),
      );
    }
    timers.push(
      window.setTimeout(
        () => startPointsPhase(),
        countdownAt + SETTLEMENT_COUNTDOWN_MS,
      ),
    );

    return () => {
      timers.forEach(window.clearTimeout);
      settlementPointTimers.current.forEach(window.clearTimeout);
      settlementPointTimers.current = [];
    };
    // The settlement key deliberately ignores confirmation updates so other
    // players confirming cannot restart the reveal animation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settlementKey]);

  /* 确认窗口剩下的时间，由服务端下发；`null` 表示窗口还没开。 */
  const settlementConfirmRemainingMs =
    matchView?.hand_settlement?.confirm_remaining_ms ?? null;

  /* 窗口一开就说明全场都播完了（掉线的那家由宽限补上）。还卡在摊牌阶段的人
     直接跳到点数动画，免得倒计时都走完了他还没看见按钮。 */
  useEffect(() => {
    if (settlementConfirmRemainingMs == null) return;
    startPointsPhase();
  }, [settlementConfirmRemainingMs, startPointsPhase]);

  useEffect(() => {
    if (!matchView?.result || matchView.hand_settlement) return;
    /* 这一份 view 不属于当前 match，是 store 里上一局的残影，不能当真跳转。 */
    if (matchView.id !== matchId) return;
    const timer = window.setTimeout(
      () => navigateTo({ kind: "result", matchId }),
      350,
    );
    return () => window.clearTimeout(timer);
  }, [matchId, matchView?.hand_settlement, matchView?.result, matchView?.id]);

  /* 投票退出：回房间只许走一次转场，来回跳一趟人眼看到的就是接连闪好几下。 */
  useEffect(() => {
    if (!matchView?.terminated_by_exit_vote) return;
    /* 这一份 view 不属于当前 match，是 store 里上一局的残影，不能当真跳转。 */
    if (matchView.id !== matchId) return;
    const roomId = matchView.room_id;
    /*
      对局都结束了，手上那份房态必然过期——`active_match_id` 还指着这一局。
      留着它，房间页一进来就照着旧的把人又送回牌桌，牌桌再送回来。
    */
    queryClient.removeQueries({ queryKey: ["room", roomId] });
    navigateTo({ kind: "room", roomId });
  }, [
    matchId,
    matchView?.id,
    matchView?.room_id,
    matchView?.terminated_by_exit_vote,
    queryClient,
  ]);

  /* 有人一直没load完，这局作废，确认之后回房间。 */
  const leaveTerminatedMatch = useCallback(() => {
    const roomId = useGameStore.getState().matchView?.room_id;
    if (!roomId) {
      navigateTo({ kind: "lobby" });
      return;
    }
    queryClient.removeQueries({ queryKey: ["room", roomId] });
    navigateTo({ kind: "room", roomId });
  }, [queryClient]);

  useEffect(() => {
    if (matchView?.exit_vote) {
      setYakuReferenceOpen(false);
    }
  }, [matchView?.exit_vote]);

  /* ── 开局素材 ─────────────────────────── */
  /* 换一局就重新load一次：这个组件在两局之间是不会重建的。 */
  useEffect(() => {
    assetsRequested.current = false;
    setAssetsLoaded(false);
  }, [matchId]);

  /*
   * 对局音乐得先load完再进牌桌，而且四家都得load完：后端在那之前一条命令都不
   * 收。素材缺失或者网太慢也不会把人卡死，`preloadMusic` 到点就当load完了，剩
   * 下的交给后端那道超时。
   */
  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    if (musicCatalog.isPending || characterCatalog.isPending) return;
    if (assetsRequested.current) return;
    assetsRequested.current = true;
    /* 四家的操作语音一起load：别人碰了杠了也要当场出声，等那会儿再去取文件
       就赶不上横幅了。同一个角色被两家选中也只load一遍。 */
    const voicePaths = matchView.players.flatMap((player) => {
      const character = player.character_id
        ? charactersById.get(player.character_id)
        : undefined;
      return character ? actionVoices(character).map((voice) => voice.path) : [];
    });
    /*
     * 不在这里把 onCommand 放进 deps，也不在 .then() 里直接调它——
     * onCommand 每次 wsState 变化（连接建立）就会重建一个新引用，导致 React
     * 先跑 cleanup（cancelled = true），再重跑 effect 时因为
     * assetsRequested.current 已是 true 而直接 return，Promise resolve 后
     * cancelled 为 true 就跳过 setAssetsLoaded，加载永远卡住。
     * 下面已有专门的 retry effect 负责补发 game.assets_ready，这里只管置位。
     */
    /* 四家的立直音乐一起预加载，有人立直时当场就能切过去。 */
    const riichiPaths = [
      ...new Set(
        matchView.players
          .map((player) => player.riichi_music_path)
          .filter((p): p is string => !!p),
      ),
    ];
    void Promise.all([
      preloadMusic(matchMusicPath, MATCH_MUSIC_LOAD_TIMEOUT_MS),
      ...riichiPaths.map((path) =>
        preloadMusic(path, MATCH_MUSIC_LOAD_TIMEOUT_MS),
      ),
      preloadVoices(voicePaths, MATCH_VOICE_LOAD_TIMEOUT_MS),
      preloadSfx(DISCARD_SFX),
      preloadSfx(SCORE_CHANGE_SFX),
      preloadSfx(HULE_FAN_OUT_SFX),
      preloadSfx(FU_APPEAR_SFX),
      preloadSfx(SCORE_APPEAR_SFX),
      preloadSfx(NEWROUND_PAIS_SFX),
      preloadSfx(MOUSECLICK_SFX),
    ]).then(() => {
      setAssetsLoaded(true);
    });
  }, [
    characterCatalog.isPending,
    charactersById,
    matchId,
    matchMusicPath,
    matchView,
    musicCatalog.isPending,
  ]);

  /*
   * 冲击麻将的杠点浮层。`last_kan.id` 是整局单调递增的，记住播过的号就不会因为
   * 视图重发（补帧、重连、别人的操作推过来的新版本）而把同一次杠重播一遍。
   *
   * 第一次拿到视图时只记号不播：刚进对局或断线重连时视图里带的是上一次杠，那一
   * 幕玩家早就看过了，再放一遍只会平白挡住半秒操作。
   */
  const lastKan = matchView?.last_kan ?? null;
  const playedKanId = useRef<number | null>(null);
  const kanBaselineMatchId = useRef<string | null>(null);
  const [playingKan, setPlayingKan] = useState<KanPointsView | null>(null);
  /*
   * 基线得在第一次拿到视图的时候就定下来，哪怕那会儿还没人杠过（`last_kan` 是
   * 空的）。以前是等 `last_kan` 有值才记号，于是整场第一次杠撞上「还没有基线」
   * 这条，四家一起被吞掉。
   */
  useEffect(() => {
    if (!matchView || kanBaselineMatchId.current === matchView.id) return;
    kanBaselineMatchId.current = matchView.id;
    playedKanId.current = matchView.last_kan?.id ?? 0;
  }, [matchView]);
  useEffect(() => {
    if (!lastKan) return;
    const played = playedKanId.current ?? 0;
    if (lastKan.id <= played) return;
    playedKanId.current = lastKan.id;
    setPlayingKan(lastKan);
  }, [lastKan]);
  const onKanPointsFinished = useCallback(() => {
    // 冲击麻将：动画播完后通知服务端，等四家都报告才摸岭上牌。
    if (matchView?.variant_kind === "impact" && playingKan != null) {
      onCommand("impact.kan_animation_played", { kan_id: playingKan.id });
    }
    setPlayingKan(null);
  }, [matchView?.variant_kind, onCommand, playingKan]);

  useEffect(() => {
    /* 杠点浮层还在播，托管的自动打牌也得等——挡人不挡自己说不通。 */
    if (!matchView || openingPhase !== "play" || playingKan) return;
    const command = automaticMatchCommand(matchView, assistSettings);
    if (!command) return;
    const key = `${matchView.id}:${matchView.hand_index}:${matchView.version}:${command.name}:${JSON.stringify(command.payload ?? null)}`;
    if (automaticCommandKey.current === key) return;
    automaticCommandKey.current = key;
    const timer = window.setTimeout(() => {
      setRiichiSelecting(false);
      onCommand(command.name, command.payload);
    }, command.delayMs);
    return () => {
      window.clearTimeout(timer);
      if (automaticCommandKey.current === key) {
        automaticCommandKey.current = null;
      }
    };
  }, [assistSettings, matchView, onCommand, openingPhase, playingKan]);

  useEffect(() => {
    if (
      !matchView ||
      !canLocalPlayerDiscard(matchView) ||
      matchView.turn_actions.riichi_discard_tile_ids.length === 0
    ) {
      setRiichiSelecting(false);
    }
  }, [matchView]);

  const onTileDiscard = useCallback(
    (tileId: number) => {
      const view = useGameStore.getState().matchView;
      if (!view || !canLocalPlayerDiscard(view)) {
        return;
      }
      const isRiichi =
        riichiSelecting &&
        view.turn_actions.riichi_discard_tile_ids.includes(tileId);
      if (riichiSelecting && !isRiichi) {
        return;
      }
      setRiichiSelecting(false);
      onCommand(
        view.variant_kind === "impact"
          ? "impact.discard"
          : isRiichi
            ? "riichi.riichi_discard"
            : "riichi.discard",
        { tile_id: tileId },
      );
    },
    [onCommand, riichiSelecting],
  );

  /* ── Render ───────────────────────────── */

  /* 云雾要一直挡到全场都load完，或者这局已经作废。 */
  useSceneReady(
    Boolean(
      error ||
        (matchView &&
          matchView.id === matchId &&
          assetsLoaded &&
          (allAssetsReady || assetsTimedOut)),
    ),
  );

  if (!matchView) {
    return (
      <div className="match-loading">
        <span className="match-loading__mark">東</span>
        <p>{error ?? "正在载入对局…"}</p>
      </div>
    );
  }

  const dice = openingDice(matchId, matchView.hand_index);
  const chiSelecting = chiSelectingVersion === matchView.version;
  const chiChoices = chiSelecting ? observerChiOptions(matchView) : [];
  /* 确认按钮只在服务端开了窗口之后出现，读的秒也是服务端下发的剩余时间。 */
  const pointsConfirmReady = settlementConfirmRemainingMs != null;
  const pointsSeconds = Math.ceil((settlementConfirmRemainingMs ?? 0) / 1000);
  const pointsAlreadyConfirmed =
    pointsConfirmed ||
    (matchView.hand_settlement?.confirmed_seats.includes(
      matchView.observer_seat,
    ) ??
      false);

  return (
    <div className="match-screen">
      <GameTable
        ref={gameTableRef}
        view={matchView}
        openingPhase={openingPhase}
        dice={dice}
        onTileDiscard={onTileDiscard}
        settlementRevealSeats={settlementRevealSeats}
        settlementWinningTileSeats={settlementWinningTileSeats}
        cameraConfig={tableCameraConfig}
        tableclothPath={tableclothPath}
        onRendererError={() =>
          setError("牌桌载入失败，请刷新后重试")
        }
      />
      <MatchStage>
        <PlayerHand2D
          view={matchView}
          openingPhase={openingPhase}
          onTileDiscard={onTileDiscard}
          riichiSelecting={riichiSelecting}
          autoSort={assistSettings.autoSort}
          onFocusedTileChange={focusTableTile}
          blocked={playingKan != null}
        />
        <MatchHud view={matchView} />
        <MatchAssistControls
          settings={assistSettings}
          onChange={setAssistSettings}
        />
        <div className="match-utility" aria-label="对局功能">
          <button
            type="button"
            onClick={() => setYakuReferenceOpen(true)}
            aria-label="帮助"
            title="帮助"
          >
            <CircleHelp aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            aria-label="设置"
            title="设置"
          >
            <Settings aria-hidden="true" />
          </button>
          {matchView.friend_match && (
            <button
              type="button"
              disabled={!matchView.can_start_exit_vote}
              /*
                这条指令没有参数，就不能带 payload：服务端的指令枚举里它是个
                单元变体，多给一个空对象整帧都解析不出来，点了等于没点。
              */
              onClick={() => onCommand("game.request_exit_vote")}
              aria-label="退出对战"
              title="退出对战"
            >
              <LogOut aria-hidden="true" />
            </button>
          )}
        </div>
        <ActionPanel
          view={matchView}
          onCommand={onCommand}
          riichiSelecting={riichiSelecting}
          onRiichiSelectingChange={setRiichiSelecting}
          chiSelecting={chiSelecting}
          onChiSelectingChange={(selecting) =>
            setChiSelectingVersion(selecting ? matchView.version : null)
          }
          skipCalls={assistSettings.skipCalls}
          blocked={playingKan != null}
        />
        {chiSelecting && (
          <ChiOptionPicker
            options={chiChoices}
            onSelect={(tileIds) => {
              setChiSelectingVersion(null);
              onCommand("riichi.chi", { tile_ids: tileIds });
            }}
            onCancel={() => setChiSelectingVersion(null)}
          />
        )}
        {playingKan && (
          <KanPointOverlay
            /* 换一次杠就换一次 key：上一幕还没退场也直接从头来，数字不会串。 */
            key={playingKan.id}
            view={matchView}
            kan={playingKan}
            onFinished={onKanPointsFinished}
          />
        )}
        {settlementPointsPhase ? (
          <PointChangeOverlay
            view={matchView}
            confirmReady={pointsConfirmReady}
            confirmed={pointsAlreadyConfirmed}
            secondsRemaining={pointsSeconds}
            onConfirm={sendSettlementConfirm}
          />
        ) : (
          <HandSettlement
            view={matchView}
            showPanel={settlementPanelVisible}
            confirmReady={settlementConfirmReady}
            secondsRemaining={settlementSeconds}
            locallyConfirmed={settlementLocallyConfirmed}
            onConfirm={startPointsPhase}
          />
        )}
        <ChatMessages view={matchView} />
        <CallBannerLayer view={matchView} banners={callBanners} />
        {wsState !== "connected" && (
          <div className="match-connection">
            {wsState === "connecting" ? "连接中" : "离线"}
          </div>
        )}
        {error && (
          <div className="match-error">{error}</div>
        )}
        {notice && <div className="match-notice">{notice}</div>}
        <ExitVotePanel
          view={matchView}
          onVote={(agree) =>
            onCommand("game.vote_exit", { agree })
          }
        />
      </MatchStage>
      {/* 帮助页压在整个对局舞台上。 */}
      {yakuReferenceOpen && (
        <YakuReferenceModal onClose={() => setYakuReferenceOpen(false)} />
      )}
      {settingsOpen && (
        <SettingsPanel onClose={() => setSettingsOpen(false)} />
      )}
      <ChatBox
        observerSeat={matchView.observer_seat}
        playerCharacterId={
          matchView.players.find(
            (p) => p.seat === matchView.observer_seat,
          )?.character_id ?? null
        }
        charactersById={charactersById}
      />
      <Modal
        open={assetsTimedOut}
        onClose={leaveTerminatedMatch}
        title="对局中断"
        dismissible={false}
      >
        <p className="match-terminated-message">
          有玩家出现网络问题，对局已终止
        </p>
        <button
          type="button"
          className="login-submit"
          onClick={leaveTerminatedMatch}
        >
          确定
        </button>
      </Modal>
    </div>
  );
}
