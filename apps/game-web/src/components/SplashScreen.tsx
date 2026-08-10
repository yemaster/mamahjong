import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LogOut, Maximize2, Minimize2 } from "lucide-react";

const splashBackground = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-splash.png`;
const splashLogo = `${import.meta.env.BASE_URL}assets/ui/mamahjong-splash-logo.png`;
const developerLogo = `${import.meta.env.BASE_URL}assets/ui/vtgame-developer-logo.png`;

interface Petal {
  id: number;
  left: string;
  delay: string;
  duration: string;
  size: string;
}

function createPetals(count: number): Petal[] {
  return Array.from({ length: count }, (_, id) => ({
    id,
    left: `${Math.random() * 100}%`,
    delay: `${Math.random() * 6}s`,
    duration: `${6 + Math.random() * 7}s`,
    size: `${7 + Math.random() * 10}px`,
  }));
}

interface Props {
  onEnter: () => void;
  onLogout: () => void;
  prepareGame: (reportProgress: (progress: number) => void) => Promise<void>;
  prepareCycle?: number;
  skipIntro?: boolean;
  welcomeName?: string;
}

const BACKGROUND_REVEAL_DURATION = 800;
const DEVELOPER_LOGO_FADE_IN_DURATION = 300;
const DEVELOPER_LOGO_HOLD_DURATION = 1400;
const DEVELOPER_LOGO_FADE_OUT_DURATION = 300;
const LOGO_PAUSE_DURATION = 500;
const GAME_LOGO_FADE_IN_DURATION = 300;
const GAME_LOGO_HOLD_DURATION = 2000;
const PROGRESS_FINISH_DURATION = 420;

export function SplashScreen({
  onEnter,
  onLogout,
  prepareGame,
  prepareCycle = 0,
  skipIntro = false,
  welcomeName,
}: Props) {
  const [progress, setProgress] = useState(0);
  const [backgroundLoaded, setBackgroundLoaded] = useState(false);
  const [developerLogoLoaded, setDeveloperLogoLoaded] = useState(skipIntro);
  const [developerLogoFading, setDeveloperLogoFading] = useState(skipIntro);
  const [gameLogoVisible, setGameLogoVisible] = useState(skipIntro);
  const [gameLogoHoldComplete, setGameLogoHoldComplete] = useState(skipIntro);
  const [loading, setLoading] = useState(false);
  const [ready, setReady] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const backgroundHandled = useRef(false);
  const petalList = useMemo(() => createPetals(28), []);
  const backgroundReady = backgroundLoaded && gameLogoHoldComplete;

  const handleBackgroundLoaded = useCallback(() => {
    if (backgroundHandled.current) return;
    backgroundHandled.current = true;
    setBackgroundLoaded(true);
  }, []);

  useEffect(() => {
    const syncFullscreen = () => setFullscreen(Boolean(document.fullscreenElement));
    syncFullscreen();
    document.addEventListener("fullscreenchange", syncFullscreen);
    return () => document.removeEventListener("fullscreenchange", syncFullscreen);
  }, []);

  useEffect(() => {
    if (skipIntro) return;
    if (!developerLogoLoaded) return;

    const developerLogoFadeOutStart =
      DEVELOPER_LOGO_FADE_IN_DURATION + DEVELOPER_LOGO_HOLD_DURATION;
    const gameLogoStart =
      developerLogoFadeOutStart +
      DEVELOPER_LOGO_FADE_OUT_DURATION +
      LOGO_PAUSE_DURATION;
    const fadeDeveloperLogoTimer = setTimeout(
      () => setDeveloperLogoFading(true),
      developerLogoFadeOutStart,
    );
    const showGameLogoTimer = setTimeout(
      () => setGameLogoVisible(true),
      gameLogoStart,
    );
    const finishGameLogoHoldTimer = setTimeout(
      () => setGameLogoHoldComplete(true),
      gameLogoStart + GAME_LOGO_FADE_IN_DURATION + GAME_LOGO_HOLD_DURATION,
    );

    return () => {
      clearTimeout(fadeDeveloperLogoTimer);
      clearTimeout(showGameLogoTimer);
      clearTimeout(finishGameLogoHoldTimer);
    };
  }, [developerLogoLoaded, skipIntro]);

  useEffect(() => {
    if (!backgroundReady) return;

    let cancelled = false;
    let finishTimer: ReturnType<typeof setTimeout> | undefined;
    const revealTimer = setTimeout(() => {
      setLoading(true);
      prepareGame((nextProgress) => {
        if (!cancelled) setProgress(nextProgress);
      })
        .then(() => {
          if (cancelled) return;
          setProgress(100);
          finishTimer = setTimeout(() => {
            if (!cancelled) setReady(true);
          }, PROGRESS_FINISH_DURATION);
        })
        .catch(() => {
          if (!cancelled) {
            setLoading(false);
            setProgress(0);
          }
        });
    }, BACKGROUND_REVEAL_DURATION);

    return () => {
      cancelled = true;
      clearTimeout(revealTimer);
      if (finishTimer) clearTimeout(finishTimer);
    };
  }, [backgroundReady, prepareCycle, prepareGame]);

  const handleEnter = () => {
    if (!ready) return;
    onEnter();
  };

  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch {
      // Browsers can reject fullscreen when it is unavailable.
    }
  };

  const handleLogout = () => {
    setReady(false);
    setLoading(false);
    setProgress(0);
    onLogout();
  };

  return (
    <div
      className={`splash-screen${developerLogoLoaded ? " is-developer-logo-visible" : ""}${developerLogoFading ? " is-developer-logo-fading" : ""}${gameLogoVisible ? " is-game-logo-visible" : ""}${backgroundReady ? " has-background" : ""}${loading ? " is-loading" : ""}${ready ? " is-ready" : ""}`}
    >
      <button
        type="button"
        className="splash-entry-hit-area"
        onClick={handleEnter}
        disabled={!ready}
        aria-label={ready ? "点击进入游戏" : `游戏加载中，${progress}%`}
      />
      <img
        className="splash-background"
        src={splashBackground}
        alt=""
        aria-hidden="true"
        onLoad={handleBackgroundLoaded}
      />
      <span className="splash-screen__veil" aria-hidden="true" />

      {welcomeName && (
        <span className="splash-welcome">欢迎您，{welcomeName}。</span>
      )}

      <span className="splash-developer-logo" aria-label="开发者标志">
        <img
          src={developerLogo}
          alt=""
          aria-hidden="true"
          onLoad={() => setDeveloperLogoLoaded(true)}
          onError={() => setDeveloperLogoLoaded(true)}
        />
      </span>

      <span className="splash-logo" aria-label="麻麻的将">
        <img
          className="splash-logo__image"
          src={splashLogo}
          alt=""
          aria-hidden="true"
        />
      </span>

      <span
        className={`splash-petals${backgroundReady ? " is-visible" : ""}`}
        aria-hidden="true"
      >
        {petalList.map((petal) => (
          <i
            key={petal.id}
            className="sakura-petal"
            style={{
              left: petal.left,
              animationDelay: petal.delay,
              animationDuration: petal.duration,
              width: petal.size,
              height: petal.size,
            }}
          />
        ))}
      </span>

      <span className="splash-bottom">
        <span
          className={`splash-loader${loading && welcomeName && !ready ? " is-visible" : ""}`}
          aria-hidden="true"
        >
          <span className="splash-loader__track">
            <span
              className="splash-loader__fill"
              style={{ width: `${progress}%` }}
            >
              <span className="splash-loader__tile">中</span>
            </span>
          </span>
        </span>
        <span
          className={`splash-enter${ready ? " is-visible" : ""}`}
          aria-hidden="true"
        >
          点击进入游戏
        </span>
      </span>

      <span
        className={`splash-actions${ready ? " is-visible" : ""}`}
        aria-hidden={!ready}
        onClick={(event) => event.stopPropagation()}
      >
        <button
          type="button"
          className="splash-action-button"
          onClick={toggleFullscreen}
          tabIndex={ready ? 0 : -1}
          aria-label={fullscreen ? "退出全屏" : "进入全屏"}
          title={fullscreen ? "退出全屏" : "进入全屏"}
        >
          {fullscreen ? <Minimize2 aria-hidden="true" /> : <Maximize2 aria-hidden="true" />}
        </button>
        <button
          type="button"
          className="splash-action-button"
          onClick={handleLogout}
          tabIndex={ready ? 0 : -1}
          aria-label="退出登录"
          title="退出登录"
        >
          <LogOut aria-hidden="true" />
        </button>
      </span>
    </div>
  );
}
