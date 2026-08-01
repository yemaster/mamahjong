import { Suspense, lazy, useEffect, useState } from "react";
import { Layout } from "./components/Layout";
import { LoginModal } from "./components/LoginModal";
import { useGameScene } from "./routing";
import { useAuthStore } from "./stores/authStore";

const LobbyScene = lazy(() => import("./scenes/LobbyScene"));
const RoomScene = lazy(() => import("./scenes/RoomScene"));
const CreateRoomScene = lazy(() => import("./scenes/CreateRoomScene"));
const MatchmakingScene = lazy(() => import("./scenes/MatchmakingScene"));
const GameScene = lazy(() => import("./scenes/GameScene"));
const ResultScene = lazy(() => import("./scenes/ResultScene"));
const ProfileScene = lazy(() => import("./scenes/ProfileScene"));

const fallback: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  height: "100%",
  color: "var(--color-text-dim)",
  fontSize: 16,
};

export function App() {
  const scene = useGameScene();
  const { isAuthenticated, token } = useAuthStore();
  const [loginOpen, setLoginOpen] = useState(false);

  useEffect(() => {
    if (!isAuthenticated) {
      setLoginOpen(true);
    }
  }, [isAuthenticated]);

  const renderScene = () => {
    switch (scene.kind) {
      case "lobby":
        return <LobbyScene />;
      case "room":
        if (!token) return <Placeholder text="请先登录" />;
        return <RoomScene roomId={scene.roomId} />;
      case "create-room":
        if (!token) return <Placeholder text="请先登录" />;
        return <CreateRoomScene />;
      case "matchmaking":
        if (!token) return <Placeholder text="请先登录" />;
        return <MatchmakingScene ticketId={scene.ticketId} />;
      case "game":
        if (!token) return <Placeholder text="请先登录" />;
        return <GameScene matchId={scene.matchId} />;
      case "result":
        return <ResultScene matchId={scene.matchId} />;
      case "profile":
        if (!token) return <Placeholder text="请先登录" />;
        return <ProfileScene />;
    }
  };

  return (
    <Layout>
      <Suspense fallback={<div style={fallback}>加载中…</div>}>
        {renderScene()}
      </Suspense>
      <LoginModal
        open={loginOpen}
        onClose={() => setLoginOpen(false)}
      />
    </Layout>
  );
}

function Placeholder({ text }: { text: string }) {
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
