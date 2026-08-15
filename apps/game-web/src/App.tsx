import { useQuery } from "@tanstack/react-query";
import { Suspense, lazy, useEffect } from "react";
import { gameApi } from "./api";
import { playMusic, resolveTrack } from "./audio/music";
import { MOUSECLICK_SFX, playSfx, preloadSfx } from "./audio/sfx";
import { Layout } from "./components/Layout";
import { FixedDomStage } from "./components/FixedDomStage";
import {
  SceneModuleLoaded,
  SceneTransition,
  useSceneReady,
} from "./components/SceneTransition";
import { navigateTo, useGameScene } from "./routing";
import { useAuthStore } from "./stores/authStore";

const LobbyScene = lazy(() => import("./scenes/LobbyScene"));
const RoomScene = lazy(() => import("./scenes/RoomScene"));
const MatchmakingScene = lazy(() => import("./scenes/MatchmakingScene"));
const GameScene = lazy(() => import("./scenes/GameScene"));
const ResultScene = lazy(() => import("./scenes/ResultScene"));
const YakuReferenceScene = lazy(() => import("./scenes/YakuReferenceScene"));
const ProfileScene = lazy(() => import("./scenes/ProfileScene"));
const TableSettingsScene = lazy(() => import("./scenes/TableSettingsScene"));
const RecordListScene = lazy(() => import("./scenes/RecordListScene"));
const ReplayScene = lazy(() => import("./scenes/ReplayScene"));

export function App() {
  const scene = useGameScene();
  const token = useAuthStore((state) => state.token);
  const userId = useAuthStore((state) => state.identity?.id);

  useEffect(() => {
    if (!token || !userId) return;
    let cancelled = false;
    void gameApi
      .activity(token)
      .then((activity) => {
        if (
          !cancelled &&
          activity.kind === "game" &&
          activity.match_id
        ) {
          navigateTo({ kind: "game", matchId: activity.match_id });
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [token, userId]);

  const lobbyMusicId = useAuthStore(
    (state) => state.identity?.profile.selected_lobby_music_id,
  );
  const matchMusicId = useAuthStore(
    (state) => state.identity?.profile.selected_match_music_id,
  );
  const { data: musicCatalog } = useQuery({
    queryKey: ["music-tracks"],
    queryFn: () => gameApi.musicTracks(),
    staleTime: 5 * 60_000,
  });
  const inMatch = scene.kind === "game";

  // 大厅循环大厅曲，对局循环对局曲，切场景时自己淡入淡出。
  useEffect(() => {
    // 曲库还没load好时不碰音乐：`resolveTrack` 拿到空数据只会返回 null，
    // `playMusic(null)` 会把当前正在放的曲子掐掉，之后也没法靠用户手势重播。
    if (!musicCatalog) return;
    const track = resolveTrack(
      musicCatalog.music_tracks,
      inMatch ? "match" : "lobby",
      inMatch ? matchMusicId : lobbyMusicId,
    );
    playMusic(track?.audio_path ?? null);
  }, [musicCatalog, inMatch, lobbyMusicId, matchMusicId]);

  /* 非对局界面的鼠标/触屏点击声属于“音效”，实时跟随选项页的音效音量。 */
  useEffect(() => {
    void preloadSfx(MOUSECLICK_SFX);
    const onClick = () => {
      if (scene.kind !== "game") {
        playSfx(MOUSECLICK_SFX);
      }
    };
    document.addEventListener("pointerdown", onClick);
    return () => document.removeEventListener("pointerdown", onClick);
  }, [scene.kind]);

  const renderScene = () => {
    switch (scene.kind) {
      case "lobby":
        return <LobbyScene />;
      case "room":
        if (!token) return <Placeholder text="请先登录" />;
        return <RoomScene roomId={scene.roomId} />;
      case "create-room":
        if (!token) return <Placeholder text="请先登录" />;
        return <LobbyScene initialMenu="create" />;
      case "matchmaking":
        if (!token) return <Placeholder text="请先登录" />;
        return <MatchmakingScene ticketId={scene.ticketId} />;
      case "game":
        if (!token) return <Placeholder text="请先登录" />;
        return <GameScene matchId={scene.matchId} />;
      case "result":
        return <ResultScene matchId={scene.matchId} />;
      case "yaku-reference":
        return <YakuReferenceScene />;
      case "table-settings":
        if (!token) return <Placeholder text="请先登录" />;
        return <TableSettingsScene />;
      case "records":
        if (!token) return <Placeholder text="请先登录" />;
        return <RecordListScene />;
      case "replay":
        if (!token) return <Placeholder text="请先登录" />;
        return <ReplayScene matchId={scene.matchId} />;
      case "profile":
        if (!token) return <Placeholder text="请先登录" />;
        return (
          <ProfileScene
            userId={scene.userId}
            initialTab={scene.tab}
            returnRoomId={scene.returnRoomId}
          />
        );
    }
  };

  const sceneKey =
    scene.kind === "room"
      ? `room:${scene.roomId}`
      : scene.kind === "matchmaking"
        ? `matchmaking:${scene.ticketId}`
        : scene.kind === "game" ||
            scene.kind === "result" ||
            scene.kind === "replay"
          ? `${scene.kind}:${scene.matchId}`
          : scene.kind;

  return (
    <SceneTransition sceneKey={sceneKey}>
      <Layout>
        <FixedDomStage>
          <Suspense fallback={<div className="scene-suspense-placeholder" />}>
            <SceneModuleLoaded>{renderScene()}</SceneModuleLoaded>
          </Suspense>
        </FixedDomStage>
      </Layout>
    </SceneTransition>
  );
}

function Placeholder({ text }: { text: string }) {
  useSceneReady(true);

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        color: "var(--color-text-dim)",
      }}
    >
      {text}
    </div>
  );
}
