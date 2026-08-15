import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { SCENE_TRANSITION_MIST } from "../sceneTransitionAssets";
import { FixedDomStage } from "./FixedDomStage";

type TransitionPhase = "idle" | "gathering" | "loading" | "revealing";

/* 雾气 CSS 在 650ms 合拢；再留 30ms 给最后一帧落定，才替换旧页面。 */
export const SCENE_GATHER_DURATION_MS = 680;
const PROGRESS_FINISH_DURATION = 420;
const REVEAL_DURATION = 700;
const transitionLoader = `${import.meta.env.BASE_URL}assets/ui/scene-transition-loader.svg`;

/** 云雾里等其他玩家时的进度：已就绪人数 / 总人数。 */
export interface WaitingPeers {
  ready: number;
  total: number;
}

interface SceneLoadSignals {
  markModuleLoaded: () => void;
  markSceneReady: () => void;
  reportWaitingPeers: (waiting: WaitingPeers | null) => void;
}

const SceneLoadContext = createContext<SceneLoadSignals>({
  markModuleLoaded: () => {},
  markSceneReady: () => {},
  reportWaitingPeers: () => {},
});

interface Props {
  sceneKey: string;
  children: ReactNode;
}

export function SceneTransition({ sceneKey, children }: Props) {
  const [displayedChildren, setDisplayedChildren] = useState(children);
  const [phase, setPhase] = useState<TransitionPhase>("idle");
  const [progress, setProgress] = useState(0);
  const [waitingPeers, setWaitingPeers] = useState<WaitingPeers | null>(null);
  const activeSceneKey = useRef(sceneKey);
  const latestChildren = useRef(children);
  const phaseRef = useRef<TransitionPhase>("idle");
  const progressRef = useRef(0);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  latestChildren.current = children;
  const sceneChanged = sceneKey !== activeSceneKey.current;

  const clearTimers = useCallback(() => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
  }, []);

  const changePhase = useCallback((nextPhase: TransitionPhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const updateProgress = useCallback((nextProgress: number) => {
    if (nextProgress <= progressRef.current) return;
    progressRef.current = nextProgress;
    setProgress(nextProgress);
  }, []);

  const markModuleLoaded = useCallback(() => {
    if (phaseRef.current !== "loading") return;
    updateProgress(55);
  }, [updateProgress]);

  const markSceneReady = useCallback(() => {
    if (phaseRef.current !== "loading" || progressRef.current === 100) return;

    updateProgress(100);
    timers.current.push(
      setTimeout(() => {
        changePhase("revealing");
        timers.current.push(
          setTimeout(() => {
            setDisplayedChildren(latestChildren.current);
            changePhase("idle");
          }, REVEAL_DURATION),
        );
      }, PROGRESS_FINISH_DURATION),
    );
  }, [changePhase, updateProgress]);

  /*
   * 报过一次人数，这一幕就一直显示人数，不再退回加载图。
   *
   * 本家先load完的那一刻才数得出人数，全场齐了这个数又会没掉，云雾却还要再遮一秒多
   * 才散——中间来回换一趟就是一次抽搐。null 一律当作「还没数出来」放过去。
   */
  const reportWaitingPeers = useCallback((waiting: WaitingPeers | null) => {
    if (!waiting) return;
    setWaitingPeers((current) =>
      current?.ready === waiting.ready && current?.total === waiting.total
        ? current
        : waiting,
    );
  }, []);

  const loadSignals = useMemo(
    () => ({ markModuleLoaded, markSceneReady, reportWaitingPeers }),
    [markModuleLoaded, markSceneReady, reportWaitingPeers],
  );

  useEffect(() => {
    if (sceneKey === activeSceneKey.current) return;

    activeSceneKey.current = sceneKey;
    clearTimers();
    progressRef.current = 0;
    setProgress(0);
    setWaitingPeers(null);
    changePhase("gathering");

    timers.current.push(
      setTimeout(() => {
        setDisplayedChildren(latestChildren.current);
        progressRef.current = 25;
        setProgress(25);
        changePhase("loading");
      }, SCENE_GATHER_DURATION_MS),
    );

    return clearTimers;
  }, [changePhase, clearTimers, sceneKey]);

  const visibleChildren =
    phase === "idle" && !sceneChanged ? children : displayedChildren;

  return (
    <SceneLoadContext.Provider value={loadSignals}>
      {visibleChildren}
      {phase !== "idle" && (
        <FixedDomStage variant="transition">
          <div
            className={`scene-transition scene-transition--${phase}`}
            role="status"
            aria-label="页面切换中"
          >
            <div className="scene-transition__white" />
            <div className="scene-transition__mist scene-transition__mist--left">
              <img
                src={SCENE_TRANSITION_MIST}
                alt=""
                aria-hidden="true"
                draggable={false}
              />
            </div>
            <div className="scene-transition__mist scene-transition__mist--right">
              <img
                src={SCENE_TRANSITION_MIST}
                alt=""
                aria-hidden="true"
                draggable={false}
              />
            </div>
            {waitingPeers ? (
              <div className="scene-transition__waiting">
                <span className="scene-transition__waiting-text">
                  等待其他玩家({waitingPeers.ready}/{waitingPeers.total})
                </span>
                <span className="scene-transition__waiting-dots" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                </span>
              </div>
            ) : (
              <div className="scene-transition__loader" aria-hidden="true">
                <img
                  className="scene-transition__loader-base"
                  src={transitionLoader}
                  alt=""
                />
                <span
                  className="scene-transition__loader-fill"
                  style={{ width: `${progress}%` }}
                >
                  <img src={transitionLoader} alt="" />
                </span>
              </div>
            )}
          </div>
        </FixedDomStage>
      )}
    </SceneLoadContext.Provider>
  );
}

export function SceneModuleLoaded({ children }: { children: ReactNode }) {
  const { markModuleLoaded } = useContext(SceneLoadContext);

  useEffect(() => {
    markModuleLoaded();
  }, [markModuleLoaded]);

  return children;
}

/**
 * 云雾里改显示「等待其他玩家(x/y)」。
 *
 * 本家已经load完、别家还没好的时候用。传 null 表示这会儿还数不出人数，不会把已经
 * 显示出来的人数撤掉——换回加载图只会闪一下。人数显示到这一幕结束为止。
 */
export function useSceneWaitingPeers(waiting: WaitingPeers | null) {
  const { reportWaitingPeers } = useContext(SceneLoadContext);
  const ready = waiting?.ready;
  const total = waiting?.total;

  useEffect(() => {
    if (ready == null || total == null) return;
    reportWaitingPeers({ ready, total });
  }, [ready, total, reportWaitingPeers]);
}

export function useSceneReady(isReady: boolean) {
  const { markSceneReady } = useContext(SceneLoadContext);

  useEffect(() => {
    if (isReady) markSceneReady();
  }, [isReady, markSceneReady]);
}
