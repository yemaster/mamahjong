interface GameSceneProps {
  matchId: string;
}

export default function GameScene({ matchId }: GameSceneProps) {
  return (
    <div style={{ padding: 40, fontSize: 16, color: "var(--color-text)" }}>
      对局 {matchId} — 即将实现
    </div>
  );
}
