import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { lazy, useCallback, useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { ApiError, gameApi, SESSION_INVALID_EVENT } from "./api";
import {
  playMusic,
  preloadMusic,
  resolveTrack,
  stopMusic,
} from "./audio/music";
import { LoginModal } from "./components/LoginModal";
import { Modal } from "./components/Modal";
import {
  SceneModuleLoaded,
  SceneTransition,
  useSceneReady,
} from "./components/SceneTransition";
import { SplashScreen } from "./components/SplashScreen";
import {
  RETURN_TO_SPLASH_EVENT,
  useAuthStore,
} from "./stores/authStore";
import type { MusicTrackListResponse } from "./types";
import { useSakuraClickEffect } from "./effects/useSakuraClickEffect";
import "./styles/global.css";

const loadApp = () => import("./App");
const loadLobby = () => import("./scenes/LobbyScene");
const LOBBY_LOGOUT_REVEAL_DELAY_MS = 1_820;
const App = lazy(() => loadApp().then((m) => ({ default: m.App })));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 15_000 },
  },
});

function Root() {
  const [showApp, setShowApp] = useState(false);
  const [loginRequired, setLoginRequired] = useState(false);
  const [splashCycle, setSplashCycle] = useState(0);
  const [prepareCycle, setPrepareCycle] = useState(0);
  const [resumeSplash, setResumeSplash] = useState(false);
  const [sessionExpired, setSessionExpired] = useState(false);
  const [suppressLogin, setSuppressLogin] = useState(false);
  const { token, identity, isAuthenticated } = useAuthStore();

  useSakuraClickEffect();

  const prepareGame = useCallback(
    async (reportProgress: (progress: number) => void) => {
      const auth = useAuthStore.getState();
      if (auth.token) {
        try {
          const user = await gameApi.me(auth.token);
          auth.setToken(auth.token);
          auth.setIdentity(user);
        } catch {
          auth.logout();
        }
      }

      const currentAuth = useAuthStore.getState();
      if (
        !currentAuth.token ||
        !currentAuth.isAuthenticated ||
        !currentAuth.identity
      ) {
        setLoginRequired(true);
        await waitForAuthentication();
        setLoginRequired(false);
      } else {
        setLoginRequired(false);
      }

      reportProgress(25);
      const authenticatedToken = useAuthStore.getState().token;
      if (!authenticatedToken) {
        throw new Error("登录状态无效");
      }

      // 大厅音乐要在进大厅之前load完，所以先把曲库取回来再排load任务。
      const lobbyMusic = queryClient
        .fetchQuery({
          queryKey: ["music-tracks"],
          queryFn: () => gameApi.musicTracks(),
          staleTime: 5 * 60_000,
        })
        .then((response) =>
          preloadMusic(
            resolveTrack(
              response.music_tracks,
              "lobby",
              useAuthStore.getState().identity?.profile
                .selected_lobby_music_id,
            )?.audio_path ?? null,
          ),
        )
        .catch(() => undefined);

      const tasks = [
        loadApp(),
        loadLobby(),
        lobbyMusic,
        queryClient.prefetchQuery({
          queryKey: ["rooms"],
          queryFn: () => gameApi.rooms(authenticatedToken),
        }),
      ];
      let completed = 0;

      await Promise.all(
        tasks.map((task) =>
          task.finally(() => {
            completed += 1;
            reportProgress(
              25 + Math.round((completed / tasks.length) * 75),
            );
          }),
        ),
      );
    },
    [],
  );

  const logoutFromSplash = useCallback(() => {
    useAuthStore.getState().logout();
    setLoginRequired(true);
    setPrepareCycle((cycle) => cycle + 1);
  }, []);

  useEffect(() => {
    let loginTimer: number | null = null;
    const returnToSplash = () => {
      // Logout immediately so the splash screen doesn't flash
      // "欢迎您，xxx" during the fog transition.
      useAuthStore.getState().logout();
      stopMusic();
      setShowApp(false);
      setResumeSplash(true);
      setLoginRequired(false);
      setSplashCycle((cycle) => cycle + 1);
      if (loginTimer != null) window.clearTimeout(loginTimer);
      loginTimer = window.setTimeout(() => {
        setLoginRequired(true);
        setPrepareCycle((cycle) => cycle + 1);
        loginTimer = null;
      }, LOBBY_LOGOUT_REVEAL_DELAY_MS);
    };
    window.addEventListener(RETURN_TO_SPLASH_EVENT, returnToSplash);
    return () => {
      window.removeEventListener(RETURN_TO_SPLASH_EVENT, returnToSplash);
      if (loginTimer != null) window.clearTimeout(loginTimer);
    };
  }, []);

  useEffect(() => {
    const invalidateSession = () => {
      if (!useAuthStore.getState().token) return;
      // Logout, start the fog transition back to splash, and show the
      // "logged in elsewhere" modal — all at once.
      useAuthStore.getState().logout();
      stopMusic();
      setShowApp(false);
      setResumeSplash(true);
      setSplashCycle((cycle) => cycle + 1);
      setSuppressLogin(true);
      setSessionExpired(true);
    };
    window.addEventListener(SESSION_INVALID_EVENT, invalidateSession);
    return () =>
      window.removeEventListener(SESSION_INVALID_EVENT, invalidateSession);
  }, []);

  useEffect(() => {
    if (!token) return;
    const timer = window.setInterval(() => {
      const currentToken = useAuthStore.getState().token;
      if (currentToken) {
        gameApi.me(currentToken).catch((error) => {
          if (
            error instanceof ApiError &&
            error.code === "auth.invalid_session"
          ) {
            window.dispatchEvent(new CustomEvent(SESSION_INVALID_EVENT));
          }
        });
      }
    }, 3000);
    return () => window.clearInterval(timer);
  }, [token]);

  const acknowledgeSessionExpiry = () => {
    setSessionExpired(false);
    setSuppressLogin(false);
    setLoginRequired(true);
  };

  return (
    <>
      <SceneTransition sceneKey={showApp ? "lobby" : "splash"}>
        {showApp ? (
          <InitialGame />
        ) : (
          <ReadySplash
            key={splashCycle}
            prepareGame={prepareGame}
            prepareCycle={prepareCycle}
            onEnter={() => {
              const currentToken = useAuthStore.getState().token;
              if (currentToken) {
                gameApi.revokeOtherSessions(currentToken).catch(() => {});
              }
              // 这一下点击就是浏览器要的用户操作，音乐只能从这里起。
              playMusic(
                resolveTrack(
                  queryClient.getQueryData<MusicTrackListResponse>([
                    "music-tracks",
                  ])?.music_tracks,
                  "lobby",
                  useAuthStore.getState().identity?.profile
                    .selected_lobby_music_id,
                )?.audio_path ?? null,
              );
              setShowApp(true);
            }}
            onLogout={logoutFromSplash}
            skipIntro={resumeSplash}
            welcomeName={
              identity?.profile.nickname || identity?.login_name
            }
          />
        )}
      </SceneTransition>
      <LoginModal
        open={
          !sessionExpired &&
          !suppressLogin &&
          (loginRequired ||
          (showApp && (!isAuthenticated || !token || !identity))
          )
        }
        onClose={() => {}}
        dismissible={false}
      />
      <Modal
        open={sessionExpired}
        onClose={acknowledgeSessionExpiry}
        title="登录已失效"
        dismissible={false}
      >
        <p className="session-expired-message">
          账号已在其他地方登录，当前设备已退出。
        </p>
        <button
          type="button"
          className="login-submit"
          onClick={acknowledgeSessionExpiry}
        >
          确定
        </button>
      </Modal>
    </>
  );
}

function ReadySplash(
  props: React.ComponentProps<typeof SplashScreen>,
) {
  useSceneReady(true);
  return <SplashScreen {...props} />;
}

function waitForAuthentication(): Promise<void> {
  const current = useAuthStore.getState();
  if (current.token && current.isAuthenticated && current.identity) {
    return Promise.resolve();
  }

  return new Promise((resolve) => {
    const unsubscribe = useAuthStore.subscribe((state) => {
      if (state.token && state.isAuthenticated && state.identity) {
        unsubscribe();
        resolve();
      }
    });
  });
}

function InitialGame() {
  useSceneReady(true);

  return (
    <QueryClientProvider client={queryClient}>
      <React.Suspense
        fallback={<div className="app-boot-screen" aria-label="正在进入游戏" />}
      >
        <SceneModuleLoaded>
          <App />
        </SceneModuleLoaded>
      </React.Suspense>
    </QueryClientProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
