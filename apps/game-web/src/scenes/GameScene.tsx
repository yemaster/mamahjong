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
  type ExchangeSnapshot,
  EXCHANGE_CINEMATIC_MS,
  GameTable,
  type GameTableHandle,
  settlementCoveringSeats,
  sortHandForDisplay,
  TSUMO_THROW_MS,
} from "../game/table";
import { CallBannerLayer } from "../game/CallBanner";
import { MatchStage } from "../game/MatchStage";
import {
  detectMeldCalls,
  detectNukiCalls,
  detectRiichiCalls,
  drawReasonLabel,
  drawRevealOrder,
  isDoubleRiichiTurn,
  type CallBannerItem,
  type CallKind,
} from "../game/callBanners";
import {
  CALL_BANNER_MS,
  EXCHANGE_INCOMING_SETTLE_MS,
  POINTS_REVEAL_MS,
  SETTLEMENT_COUNTDOWN_MS,
  SETTLEMENT_REVEAL_BUDGET_MS,
} from "../game/animationTiming";
import { ActionPanel } from "../game/ActionPanel";
import { SettingsPanel } from "../game/SettingsPanel";
import { ChatBox, ChatMessages } from "../game/ChatBox";
import { ChiOptionPicker } from "../game/ChiOptionPicker";
import { chiCommandName, observerChiOptions } from "../game/chiOptions";
import { KanOptionPicker } from "../game/KanOptionPicker";
import { kanCommand, observerKanOptions } from "../game/kanOptions";
import { commandRejectionText } from "../game/commandErrors";
import { ExitVotePanel } from "../game/ExitVotePanel";
import { MatchHud } from "../game/MatchHud";
import { HandSettlement } from "../game/HandSettlement";
import { KanPointOverlay } from "../game/KanPointOverlay";
import { SichuanWinOverlay } from "../game/SichuanWinOverlay";
import { PointChangeOverlay } from "../game/PointChangeOverlay";
import { MatchAssistControls } from "../game/MatchAssistControls";
import { PlayerHand2D } from "../game/PlayerHand2D";
import { SichuanPhaseOverlay } from "../game/SichuanPhase";
import { applyViewPatch } from "../game/viewPatch";
import {
  advanceTileCode,
  DEV_HAND_KEYS,
  isDevModeEnabled,
  validTileCodes,
} from "../game/devMode";
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
import { useChatStore } from "../stores/chatStore";
import { useGameStore } from "../stores/gameStore";
import { navigateTo } from "../routing";
import type {
  GameCommandName,
  KanPointsView,
  MatchView,
  SichuanWinView,
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
  const setLatency = useGameStore((state) => state.setLatency);
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
  const [kanSelectingVersion, setKanSelectingVersion] = useState<number | null>(
    null,
  );
  /*
   * 四川换三张：提交那一刻抓下的换前快照、以及本地动画播完的标记。选牌在二维
   * 面板里完成，点「换牌」才把快照定格——第四家交牌时服务端当场就换好了牌，
   * 视图里已是换后的手牌，桌上那段飞出/换位/飞入的演出得照着快照来。
   */
  const [exchangeSnapshot, setExchangeSnapshot] =
    useState<ExchangeSnapshot | null>(null);
  const exchangePreHandSnapshot = useRef<ExchangeSnapshot | null>(null);
  const [exchangeAnimDone, setExchangeAnimDone] = useState(false);
  const [exchangeIncomingTileIds, setExchangeIncomingTileIds] = useState<number[]>(
    [],
  );
  const exchangeHiddenTileIds = useMemo(() => {
    if (!exchangeSnapshot || !matchView) return [];
    const preHandIds = new Set(exchangeSnapshot.hand.map((tile) => tile.id));
    const observer = matchView.players.find(
      (player) => player.seat === matchView.observer_seat,
    );
    const incomingIds = (observer?.concealed_tiles ?? [])
      .filter((tile) => !preHandIds.has(tile.id))
      .map((tile) => tile.id);
    return [...exchangeSnapshot.outgoingIds, ...incomingIds];
  }, [exchangeSnapshot, matchView]);
  const exchangeHandKey = useRef<string | null>(null);
  /* 回执只发一次的幂等闸、以及演出卡壳时的兜底计时器。 */
  const exchangeAckSent = useRef(false);
  const exchangeSafetyTimer = useRef<number | null>(null);
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

  /* 换入牌只在动画交接后的这一帧抬起一次。若一直保留这些 id，后续每次
     出牌造成手牌 DOM 重排时，CSS 类会重新挂载，三张牌就会反复插入。 */
  useEffect(() => {
    if (exchangeIncomingTileIds.length === 0) return;
    const timer = window.setTimeout(
      () => setExchangeIncomingTileIds([]),
      EXCHANGE_INCOMING_SETTLE_MS,
    );
    return () => window.clearTimeout(timer);
  }, [exchangeIncomingTileIds]);

  /* 换三张的本地状态同样一局一清，上一局的快照/回执闸不能带进新一局。 */
  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    const handKey = `${matchView.id}:${matchView.hand_index}`;
    if (exchangeHandKey.current === handKey) return;
    exchangeHandKey.current = handKey;
    setExchangeSnapshot(null);
    exchangePreHandSnapshot.current = null;
    setExchangeAnimDone(false);
    setExchangeIncomingTileIds([]);
    exchangeAckSent.current = false;
    if (exchangeSafetyTimer.current != null) {
      window.clearTimeout(exchangeSafetyTimer.current);
      exchangeSafetyTimer.current = null;
    }
  }, [matchId, matchView?.hand_index, matchView?.id]);

  /* 在换牌窗口刚打开时保存换前手牌。超时自动换牌或断线重连时，服务端只下发
     交出的牌号，客户端仍能按同一份换前快照完成三维演出。 */
  useEffect(() => {
    if (
      !matchView ||
      matchView.variant_kind !== "sichuan" ||
      matchView.phase.kind !== "awaiting_exchange" ||
      exchangePreHandSnapshot.current != null
    ) {
      return;
    }
    const observer = matchView.players.find(
      (player) => player.seat === matchView.observer_seat,
    );
    if (!observer?.concealed_tiles) return;
    exchangePreHandSnapshot.current = {
      handKey: `${matchView.id}:${matchView.hand_index}`,
      hand: sortHandForDisplay(observer.concealed_tiles, null).map((tile) => ({
        id: tile.id,
        code: tile.code,
      })),
      outgoingIds: [],
    };
  }, [matchView]);

  /* 代打/超时的玩家没有经过二维确认，收到服务端保存的三张交牌后也要播完整换牌。 */
  useEffect(() => {
    if (
      !matchView ||
      matchView.variant_kind !== "sichuan" ||
      (matchView.phase.kind !== "awaiting_exchange_animation" &&
        matchView.phase.kind !== "awaiting_dingque") ||
      exchangeSnapshot != null
    ) {
      return;
    }
    const preHand = exchangePreHandSnapshot.current;
    const outgoingIds = matchView.exchange_outgoing_tile_ids;
    if (!preHand || outgoingIds?.length !== 3) return;
    setExchangeSnapshot({
      handKey: `${matchView.id}:${matchView.hand_index}`,
      hand: preHand.hand,
      outgoingIds: [...outgoingIds],
    });
  }, [exchangeSnapshot, matchView]);

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
    useChatStore.getState().clear();
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
              case "latency":
                setLatency(event.milliseconds);
                break;
              case "chat":
                useChatStore
                  .getState()
                  .receive(event.seat, event.messageType, event.content);
                break;
              case "disconnected":
                setLatency(null);
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
      useChatStore.getState().clear();
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
        if (streamRef.current.sendCommand(name, payload, view.version)) {
          return;
        }
        /* 连接状态刚标成 connected、底层 socket 却已关闭的空窗里，别把这条
           指令丢掉——尤其 impact.kan_animation_played 这种一次性的握手，
           掉了服务端会一直卡到超时。退回去走 HTTP 保证它一定送到。 */
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
  const sendChat = useCallback(
    (type: "text" | "emoji", content: string) => {
      if (wsState !== "connected") return false;
      return streamRef.current?.sendChat(type, content) ?? false;
    },
    [wsState],
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
      const show = () => {
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
      };
      if (delay <= 0) show();
      else bannerTimers.current.push(window.setTimeout(show, delay));
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
    for (const seat of detectNukiCalls(matchView, previous)) {
      pushBanner("nuki", seat);
      shout(seat, "nuki");
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

  /* 启动点数动画。普通结算在点数滚完后报告播完；四川“已有胡家后流局”
     还要继续展示胡牌结算页，因此由最后一页负责回执。 */
  const beginPointsPhase = useCallback((reportPlayed: boolean) => {
    const view = useGameStore.getState().matchView;
    if (!view?.hand_settlement || pointsPhaseStarted.current) {
      return;
    }
    pointsPhaseStarted.current = true;
    setSettlementLocallyConfirmed(true);
    setSettlementConfirmReady(false);
    setSettlementPointsPhase(true);
    setPointsConfirmed(false);
    if (reportPlayed) {
      settlementPointTimers.current.push(
        window.setTimeout(() => sendSettlementPlayed(), POINTS_REVEAL_MS),
      );
    }
  }, [sendSettlementPlayed]);

  const startPointsPhase = useCallback(
    () => beginPointsPhase(true),
    [beginPointsPhase],
  );

  /* 四川“已有胡家后流局”：点数结束后还要逐页展示胡家，不能在点数阶段结束时
     就报告整段结算完成；最后一位胡家页面播完后再由 HandSettlement 回执。 */
  const startPointsBeforeWinnerSettlement = useCallback(
    () => beginPointsPhase(false),
    [beginPointsPhase],
  );

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
    const isSichuan = matchView.variant_kind === "sichuan";
    const isDraw = settlement.reason === "exhaustive_draw";

    /* 四川和牌时的语音、胡牌动画已经在每次胡牌事件到达时完成。最终结算只
       保留胡牌结算界面，不重复播点数/胡牌动画；等结算页自身的番种/分数展示
       播完后再把回执交给后端，由服务端开启统一确认倒计时。 */
    if (isSichuan && !isDraw) {
      setSettlementPanelVisible(true);
      return () => {
        settlementPointTimers.current.forEach(window.clearTimeout);
        settlementPointTimers.current = [];
      };
    }

    const timers: number[] = [];
    const revealSeat = (seat: number) =>
      setSettlementRevealSeats((current) =>
        current.includes(seat) ? current : [...current, seat],
      );
    /* 四川麻将每家单独记自摸/荣和；其余两家整局只有一种，直接看 reason。 */
    const winnerIsTsumo = (seat: number) =>
      settlement.winners.find((winner) => winner.seat === seat)?.is_tsumo ??
      settlement.reason === "tsumo";

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
      const order = drawRevealOrder(matchView).filter(
        (seat) =>
          !isSichuan ||
          !matchView.players.find((player) => player.seat === seat)?.won,
      );
      const firstAt = 900;
      const stepMs = 500;
      /* 四家摊完之后留一拍，再进点数动画。摊牌阶段
         有硬上界，服务端的兜底就是照这个上界算出来的。 */
      const pointsAt = Math.min(
        firstAt + (order.length - 1) * stepMs + 1800,
        SETTLEMENT_REVEAL_BUDGET_MS,
      );
      const isImpactDraw = matchView.variant_kind === "impact";
      /* 四川麻将听牌家记在 `que.tenpai`，立直记在 `tenpai_seats`。 */
      const tenpaiSeats = isSichuan
        ? (settlement.que?.tenpai ?? [])
        : settlement.tenpai_seats;
      order.forEach((seat, index) => {
        const at = firstAt + index * stepMs;
        timers.push(window.setTimeout(() => revealSeat(seat), at));
        /* 冲击麻将荒牌流局不区分听/不听，直接摊牌进点数。 */
        if (!isImpactDraw) {
          const tenpai = tenpaiSeats.includes(seat);
          /* 花猪（手牌含三门）单独点名，和普通未听区分开。 */
          const pig =
            isSichuan && (settlement.que?.flower_pigs ?? []).includes(seat);
          /* 听牌者旁边直接摆出听的牌，不听的就挂个「不听」；两者都留到
             点数动画开始才收走，方便一眼看完全场。 */
          const waits = tenpai
            ? (matchView.players.find((player) => player.seat === seat)
                ?.waiting_tiles ?? [])
            : undefined;
          const shownAt = at + 160;
          if (pig) {
            pushBanner(
              "noten",
              seat,
              shownAt,
              Math.max(600, pointsAt - shownAt),
              undefined,
              true,
              "花猪",
            );
          } else {
            pushBanner(
              tenpai ? "tenpai" : "noten",
              seat,
              shownAt,
              Math.max(600, pointsAt - shownAt),
              waits,
              true,
            );
          }
        }
      });
      /* 四川流局：听牌结果展示后立即进入点数动画；没有胡牌结算页需要用户逐页确认，
         点数动画报告完成后由客户端自动确认并开始下一局；本局已有胡家时，点数动画
         结束后再依次展示胡家结算页。 */
      timers.push(
        window.setTimeout(() => {
          const hasSichuanWinners = isSichuan && winnerSeats.length > 0;
          if (hasSichuanWinners) {
            startPointsBeforeWinnerSettlement();
            timers.push(
              window.setTimeout(() => {
                setSettlementPointsPhase(false);
                setSettlementLocallyConfirmed(false);
                setSettlementPanelVisible(true);
              }, POINTS_REVEAL_MS),
            );
          } else {
            startPointsPhase();
          }
        }, pointsAt),
      );
      return () => {
        timers.forEach(window.clearTimeout);
        settlementPointTimers.current.forEach(window.clearTimeout);
        settlementPointTimers.current = [];
      };
    }

    // 自摸 / 荣和: shout the call beside the winner, lay the hand out, then
    // turn over whoever has to cover. 血战到底多家胡时逐家来。
    let at = 300;
    winnerSeats.forEach((seat) => {
      const tsumo = winnerIsTsumo(seat);
      /* 四川麻将已经在每次胡牌的即时点数动画前播报；结算页不再重复喊。 */
      if (!isSichuan) {
        pushBanner(tsumo ? "tsumo" : "ron", seat, Math.max(0, at - 300), 1600);
        shout(seat, tsumo ? "tsumo" : "ron", Math.max(0, at - 300));
      }
      if (tsumo) {
        /* 自摸先把那张牌从高处砸到桌上，等那一下的灰扬起来，手牌才跟着瘫下去。 */
        timers.push(
          window.setTimeout(() => {
            setSettlementWinningTileSeats((current) =>
              current.includes(seat) ? current : [...current, seat],
            );
          }, at),
        );
        at += TSUMO_THROW_MS + 200;
      }
      timers.push(window.setTimeout(() => revealSeat(seat), at));
      at += 320;
    });
    const winnerRevealEnd = at;

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
    if (
      matchView?.variant_kind === "sichuan" &&
      (matchView.hand_settlement?.winners.length ?? 0) === 0
    ) {
      // 四川流局没有胡牌结算页：听牌结果与点数动画结束后直接开下一局。
      sendSettlementConfirm();
      return;
    }
    if (
      matchView?.variant_kind === "sichuan" &&
      (matchView.hand_settlement?.winners.length ?? 0) > 0
    ) {
      // 流局点数动画后才显示胡家结算页；确认窗口由服务端统一开启。
      setSettlementConfirmReady(true);
      return;
    }
    startPointsPhase();
  }, [matchView, sendSettlementConfirm, settlementConfirmRemainingMs, startPointsPhase]);

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
  const kanOverlayHandIndex = useRef<number | null>(null);
  const [playingKan, setPlayingKan] = useState<KanPointsView | null>(null);
  const [sichuanWin, setSichuanWin] = useState<SichuanWinView | null>(null);
  const [sichuanWinRevealSeats, setSichuanWinRevealSeats] = useState<number[]>(
    [],
  );
  const playedSichuanWinId = useRef<number | null>(null);
  const sichuanWinAckSent = useRef(false);
  const sichuanWinHandKey = useRef<string | null>(null);
  /*
   * 基线得在第一次拿到视图的时候就定下来，哪怕那会儿还没人杠过（`last_kan` 是
   * 空的）。以前是等 `last_kan` 有值才记号，于是整场第一次杠撞上「还没有基线」
   * 这条，四家一起被吞掉。
   */
  useEffect(() => {
    if (!matchView || kanBaselineMatchId.current === matchView.id) return;
    kanBaselineMatchId.current = matchView.id;
    /*
     * 若进入时正处于 awaiting_kan_animation，说明这次杠的浮层还没播过——不能把
     * 当前 last_kan.id 纳入"已播"基线，否则第二个 effect 看到 id <= played 就
     * 直接跳过，覆层永不出现，impact.kan_animation_played 也就永远发不出去，服
     * 务端会一直卡在等待状态直到超时。把基线设成 id - 1，让这次杠正常触发。
     */
    const awaitingKan = matchView.phase.kind === "awaiting_kan_animation";
    playedKanId.current = awaitingKan
      ? Math.max(0, (matchView.last_kan?.id ?? 1) - 1)
      : (matchView.last_kan?.id ?? 0);
    kanOverlayHandIndex.current = matchView.hand_index;
  }, [matchView]);
  useEffect(() => {
    if (!matchView) return;
    const previousHand = kanOverlayHandIndex.current;
    kanOverlayHandIndex.current = matchView.hand_index;
    if (previousHand != null && previousHand !== matchView.hand_index) {
      /* 上一局没来得及退场的杠点浮层绝不能挡住新一局的副露/荣和按钮。 */
      setPlayingKan(null);
    }
  }, [matchView?.hand_index]);
  useEffect(() => {
    if (!lastKan) return;
    const played = playedKanId.current ?? 0;
    if (lastKan.id <= played) return;
    playedKanId.current = lastKan.id;
    setPlayingKan(lastKan);
  }, [lastKan]);
  const onKanPointsFinished = useCallback(() => {
    if (playingKan != null) {
      if (
        matchView?.variant_kind === "impact" &&
        playingKan.kind !== "chankan"
      ) {
        // 冲击麻将：动画播完后通知服务端，等四家都报告才摸岭上牌。
        onCommand("impact.kan_animation_played", { kan_id: playingKan.id });
      } else if (matchView?.variant_kind === "sichuan") {
        onCommand("sichuan.kan_animation_played", { kan_id: playingKan.id });
      }
    }
    setPlayingKan(null);
  }, [matchView?.variant_kind, onCommand, playingKan]);

  /* 四川胡牌：事件先锁住整桌，点数滚完再放行牌桌盖牌/亮胡张，最后向后端回执。 */
  useEffect(() => {
    if (!matchView || matchView.variant_kind !== "sichuan") return;
    const event = matchView.last_win;
    if (!event) return;
    if (playedSichuanWinId.current == null) {
      playedSichuanWinId.current =
        matchView.phase.kind === "awaiting_win_animation" ? event.id - 1 : event.id;
    }
    if (
      matchView.phase.kind !== "awaiting_win_animation" ||
      event.id <= (playedSichuanWinId.current ?? 0)
    ) {
      return;
    }
    playedSichuanWinId.current = event.id;
    sichuanWinAckSent.current = false;
    setSichuanWinRevealSeats([]);
    /* 胡牌事件到达即播报，点数动画挂载前完成横幅与语音触发。 */
    pushBanner(event.is_tsumo ? "tsumo" : "ron", event.seat, 0, 1600);
    shout(event.seat, event.is_tsumo ? "tsumo" : "ron");
    setSichuanWin(event);
  }, [matchView, pushBanner, shout]);

  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    if (matchView.phase.kind === "awaiting_win_animation") return;
    /* 进入下一阶段后保留盖牌状态，但不再把上一局事件当成新事件。 */
    if (sichuanWin == null) return;
    setSichuanWin(null);
  }, [matchId, matchView, sichuanWin]);

  useEffect(() => {
    if (!matchView || matchView.id !== matchId) return;
    const handKey = `${matchView.id}:${matchView.hand_index}`;
    if (sichuanWinHandKey.current === handKey) return;
    sichuanWinHandKey.current = handKey;
    setSichuanWinRevealSeats([]);
    /* 直接重连到等待胡牌动画时，上一 effect 已经拿到本次事件；不能在这里把它清掉。 */
    const pendingWin =
      matchView.phase.kind === "awaiting_win_animation" &&
      matchView.last_win != null;
    if (!pendingWin) {
      setSichuanWin(null);
      playedSichuanWinId.current = null;
      sichuanWinAckSent.current = false;
    }
  }, [matchId, matchView?.hand_index, matchView?.id]);

  const revealSichuanWin = useCallback(() => {
    const event = useGameStore.getState().matchView?.last_win;
    if (!event) return;
    setSichuanWinRevealSeats((current) =>
      current.includes(event.seat) ? current : [...current, event.seat],
    );
  }, []);

  const finishSichuanWin = useCallback(() => {
    if (sichuanWinAckSent.current) return;
    const event = useGameStore.getState().matchView?.last_win;
    if (!event) return;
    sichuanWinAckSent.current = true;
    onCommand("sichuan.win_animation_played", { win_id: event.id });
    setSichuanWin(null);
  }, [onCommand]);

  useEffect(() => {
    /* 点数/胡牌/杠点浮层还在播，托管的自动打牌也得等。 */
    if (!matchView || openingPhase !== "play" || playingKan || sichuanWin) return;
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
  }, [assistSettings, matchView, onCommand, openingPhase, playingKan, sichuanWin]);

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
          : view.variant_kind === "sichuan"
            ? "sichuan.discard"
            : isRiichi
              ? "riichi.riichi_discard"
              : "riichi.discard",
        { tile_id: tileId },
      );
    },
    [onCommand, riichiSelecting],
  );

  /* ── 四川换三张 ─────────────────────── */
  /*
   * 回执只发一次：演出正常播完、刷新补发、兜底计时器到点，三条路都汇聚到这里，
   * 用 ref 保证幂等，绝不多报。
   */
  const completeExchangeLocally = useCallback((view: MatchView | null) => {
    if (!view) return;
    if (exchangeSafetyTimer.current != null) {
      window.clearTimeout(exchangeSafetyTimer.current);
      exchangeSafetyTimer.current = null;
    }
    const preHandIds = new Set(
      exchangePreHandSnapshot.current?.hand.map((tile) => tile.id) ?? [],
    );
    if (view?.variant_kind === "sichuan" && preHandIds.size > 0) {
      const observer = view.players.find(
        (player) => player.seat === view.observer_seat,
      );
      const incomingIds = (observer?.concealed_tiles ?? [])
        .filter((tile) => !preHandIds.has(tile.id))
        .map((tile) => tile.id);
      setExchangeIncomingTileIds(incomingIds.slice(0, 3));
    }
    setExchangeAnimDone(true);
    /* 快照用完即清：不清的话下一帧 updateExchange 会拿它把演出再起一遍。 */
    setExchangeSnapshot(null);
  }, []);

  const sendExchangePlayed = useCallback(() => {
    if (exchangeAckSent.current) return;
    exchangeAckSent.current = true;
    completeExchangeLocally(useGameStore.getState().matchView);
    onCommand("sichuan.exchange_animation_played");
  }, [completeExchangeLocally, onCommand]);

  /* 提交换牌（二维选牌面板交上来的三张）：定格换前快照，桌上的演出照它播。 */
  const confirmExchange = useCallback(
    (tileIds: number[]) => {
      const view = useGameStore.getState().matchView;
      if (!view || view.phase.kind !== "awaiting_exchange") return;
      if (tileIds.length !== 3) return;
      const observer = view.players.find(
        (player) => player.seat === view.observer_seat,
      );
      const hand = sortHandForDisplay(
        observer?.concealed_tiles ?? [],
        null,
      ).map((tile) => ({ id: tile.id, code: tile.code }));
      const snapshot = {
        handKey: `${view.id}:${view.hand_index}`,
        hand,
        outgoingIds: [...tileIds],
      };
      exchangePreHandSnapshot.current = snapshot;
      setExchangeSnapshot(snapshot);
      onCommand("sichuan.exchange", { tile_ids: tileIds });
    },
    [onCommand],
  );

  /* 桌上整段换牌动画播完：报告服务端，同时本地放行定缺面板与二维手牌。 */
  const handleExchangeAnimationDone = useCallback(() => {
    sendExchangePlayed();
  }, [sendExchangePlayed]);

  /*
   * 没有演出可播时（刷新直接落在定缺阶段、换牌超时被代打）立刻补回执，
   * 免得卡住定缺；服务端自己也留了兜底时限。
   */
  useEffect(() => {
    if (!matchView || matchView.variant_kind !== "sichuan") return;
    if (
      matchView.phase.kind !== "awaiting_exchange_animation" &&
      matchView.phase.kind !== "awaiting_dingque"
    ) {
      return;
    }
    const serverReleased = (
      matchView.exchange_animation_played_seats ?? []
    ).includes(matchView.observer_seat);
    /*
     * 重连时可能已经错过了换前快照，因而没有可播的三维演出。此时不要把自己
     * 永久留在动画闸门里，直接补回执；服务端阶段仍是唯一可信的定缺入口。
     */
    if (
      matchView.phase.kind === "awaiting_exchange_animation" &&
      exchangeSnapshot == null &&
      !serverReleased
    ) {
      /* 超时自动换牌也会先进入这里：上一 effect 已经保存了换前三维快照，
         但 React 还没来得及把服务端下发的 outgoing ids 组装成 snapshot。
         不能在这个中间帧抢先回执，否则下一帧只能直接显示换后手牌，整段演出
         会被跳过。等快照恢复 effect 在下一次渲染中接管；若确实没有快照，
         再走下面的无演出兜底。 */
      if (exchangePreHandSnapshot.current != null) return;
      sendExchangePlayed();
      return;
    }
    if (matchView.phase.kind !== "awaiting_dingque") return;
    if (serverReleased) {
      /* 后端动画超时会直接把所有座位放行；此时不能再等本地计时器。 */
      exchangeAckSent.current = true;
      if (exchangeSnapshot != null && !exchangeAnimDone) {
        completeExchangeLocally(matchView);
      }
      return;
    }
    if (exchangeSnapshot != null) return; // 有演出，走演出/兜底两条路
    sendExchangePlayed();
  }, [
    completeExchangeLocally,
    exchangeAnimDone,
    matchView,
    exchangeSnapshot,
    sendExchangePlayed,
  ]);

  /*
   * 兜底：快照在（演出该播）但迟迟没收到播完回调时，进入定缺阶段后等够一段
   * 就照样回执，绝不让定缺面板因为演出卡壳而一直不亮。正常情况演出自己先报，
   * 这里不会触发。
   */
  useEffect(() => {
    if (!matchView || matchView.variant_kind !== "sichuan") return;
    if (matchView.phase.kind !== "awaiting_dingque") return;
    if (exchangeSnapshot == null) return;
    if (exchangeAckSent.current) return;
    if (
      (matchView.exchange_animation_played_seats ?? []).includes(
        matchView.observer_seat,
      )
    ) {
      return;
    }
    if (exchangeSafetyTimer.current != null) return;
    exchangeSafetyTimer.current = window.setTimeout(() => {
      exchangeSafetyTimer.current = null;
      sendExchangePlayed();
    }, EXCHANGE_CINEMATIC_MS + 1500);
  }, [matchView, exchangeSnapshot, sendExchangePlayed]);

  /* ── 开发模式：改手牌 ─────────────────── */
  /*
   * 只在 `MAMAHJONG_DEV_MODE` 打开的构建里挂监听。q..f 依次对应暗手第 1..14 张
   * （显示顺序，刚摸上来的那张在末尾），按一下就把那张牌在当前牌山的牌码循环里
   * 推进一张。按键超出当前暗手范围（副露之后更少）就无视。改动走服务端的
   * `/dev/hand` 接口落到权威状态，改的是牌面、牌 id 不变，所以轮到自己的 14 张
   * 也不会凭空少一张。
   */
  useEffect(() => {
    if (!isDevModeEnabled()) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }
      const keyIndex = DEV_HAND_KEYS.indexOf(event.key.toLowerCase());
      if (keyIndex < 0) return;
      const view = useGameStore.getState().matchView;
      if (!view || view.id !== matchId) return;
      const player = view.players.find((candidate) => candidate.seat === view.observer_seat);
      if (!player?.concealed_tiles) return;
      const drawnId = player.drawn_tile_id ?? null;
      /* 存储顺序的暗手（含刚摸上来的那张），和服务端改牌的遍历顺序一致。 */
      const stored = player.concealed_tiles;
      /* 显示顺序的暗手，按键位置对的是这一份；摸上来的那张排在末尾。 */
      const displayed = sortHandForDisplay(
        player.concealed_tiles,
        drawnId,
        view.joker_code,
      );
      const targetId = displayed[keyIndex]?.id;
      if (targetId == null) return;
      const valid = validTileCodes(
        view.variant_kind,
        view.sanma_north_rule != null,
      );
      const tiles = stored.map((tile) =>
        tile.id === targetId ? advanceTileCode(tile.code, valid) : tile.code,
      );
      if (!token) return;
      gameApi
        .setDevHand(matchId, tiles, token)
        .then((next) => setMatchView(next))
        .catch((err: unknown) => setError(apiFailure(err).message));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [matchId, token, setMatchView]);

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

  /* 四川换三张方向由后端真实骰子决定，开场模型也必须展示同一组点数。其余规则
     暂时沿用既有的确定性开场骰子。 */
  const dice =
    matchView.variant_kind === "sichuan" && matchView.exchange_dice
      ? matchView.exchange_dice
      : openingDice(matchId, matchView.hand_index);
  const chiSelecting = chiSelectingVersion === matchView.version;
  const chiChoices = chiSelecting ? observerChiOptions(matchView) : [];
  const kanSelecting = kanSelectingVersion === matchView.version;
  const kanChoices = kanSelecting ? observerKanOptions(matchView) : [];
  /* 确认按钮只在服务端开了窗口之后出现，读的秒也是服务端下发的剩余时间。 */
  const pointsConfirmReady = settlementConfirmRemainingMs != null;
  const pointsSeconds = Math.ceil((settlementConfirmRemainingMs ?? 0) / 1000);
  const pointsAlreadyConfirmed =
    pointsConfirmed ||
    (matchView.hand_settlement?.confirmed_seats.includes(
      matchView.observer_seat,
    ) ??
      false);
  const finalSichuanWinSettlement =
    matchView.variant_kind === "sichuan" &&
    matchView.hand_settlement != null &&
    matchView.hand_settlement.winners.length > 0;
  const sichuanExhaustiveDraw =
    matchView.variant_kind === "sichuan" &&
    matchView.hand_settlement?.reason === "exhaustive_draw";
  /* 四川非流局结算直接显示胡牌页；“已有胡家后流局”必须先允许流局点数浮层。 */
  const showSettlementPoints =
    settlementPointsPhase &&
    (!finalSichuanWinSettlement || sichuanExhaustiveDraw);

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
        sichuanWinRevealSeats={sichuanWinRevealSeats}
        cameraConfig={tableCameraConfig}
        tableclothPath={tableclothPath}
        exchangeSnapshot={exchangeSnapshot}
        onExchangeAnimationDone={handleExchangeAnimationDone}
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
          blocked={playingKan != null || sichuanWin != null}
          exchangeAnimationDone={exchangeAnimDone}
          exchangeLocallySubmitted={exchangeSnapshot != null}
          exchangeHiddenTileIds={exchangeAnimDone ? [] : exchangeHiddenTileIds}
          exchangeCollapseTileIds={
            exchangeAnimDone ? [] : exchangeSnapshot?.outgoingIds ?? []
          }
          exchangeHandOverride={exchangeSnapshot?.hand ?? null}
          exchangeIncomingTileIds={exchangeIncomingTileIds}
          sichuanWinRevealSeats={sichuanWinRevealSeats}
        />
        <MatchHud view={matchView} />
        <SichuanPhaseOverlay
          view={matchView}
          openingPhase={openingPhase}
          onCommand={onCommand}
          onConfirmExchange={confirmExchange}
          exchangeLocallySubmitted={exchangeSnapshot != null}
          exchangeAnimationDone={exchangeAnimDone}
        />
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
          kanSelecting={kanSelecting}
          onKanSelectingChange={(selecting) =>
            setKanSelectingVersion(selecting ? matchView.version : null)
          }
          skipCalls={assistSettings.skipCalls}
          blocked={playingKan != null || sichuanWin != null}
        />
        {chiSelecting && (
          <ChiOptionPicker
            options={chiChoices}
            onSelect={(tileIds) => {
              setChiSelectingVersion(null);
              onCommand(chiCommandName(matchView), { tile_ids: tileIds });
            }}
            onCancel={() => setChiSelectingVersion(null)}
          />
        )}
        {kanSelecting && (
          <KanOptionPicker
            options={kanChoices}
            onSelect={(option) => {
              setKanSelectingVersion(null);
              const command = kanCommand(matchView, option);
              onCommand(command.name, command.payload);
            }}
            onCancel={() => setKanSelectingVersion(null)}
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
        {sichuanWin && matchView.phase.kind === "awaiting_win_animation" && (
          <SichuanWinOverlay
            key={sichuanWin.id}
            view={matchView}
            win={sichuanWin}
            onReveal={revealSichuanWin}
            onFinished={finishSichuanWin}
          />
        )}
        {showSettlementPoints ? (
          <PointChangeOverlay
            view={matchView}
            pointDeltas={
              sichuanExhaustiveDraw
                ? matchView.hand_settlement?.que?.deltas
                : undefined
            }
            confirmReady={pointsConfirmReady}
            confirmed={pointsAlreadyConfirmed}
            secondsRemaining={pointsSeconds}
            onConfirm={sendSettlementConfirm}
          />
        ) : (
          <HandSettlement
            view={matchView}
            showPanel={
              settlementPanelVisible ||
              (finalSichuanWinSettlement && !sichuanExhaustiveDraw)
            }
            confirmReady={settlementConfirmReady}
            secondsRemaining={settlementSeconds}
            locallyConfirmed={settlementLocallyConfirmed}
            onPlayed={
              finalSichuanWinSettlement ? sendSettlementPlayed : undefined
            }
            onConfirm={
              matchView.variant_kind === "sichuan" &&
              matchView.hand_settlement?.winners.length
                ? sendSettlementConfirm
                : startPointsPhase
            }
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
        playerCharacterId={
          matchView.players.find(
            (p) => p.seat === matchView.observer_seat,
          )?.character_id ?? null
        }
        charactersById={charactersById}
        connected={wsState === "connected"}
        onSend={sendChat}
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
