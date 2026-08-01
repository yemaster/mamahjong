interface ResultSceneProps {
  matchId: string;
}

export default function ResultScene({ matchId }: ResultSceneProps) {
  return (
    <div style={{ padding: 40, fontSize: 16, color: "var(--color-text)" }}>
      结算 {matchId} — 即将实现
    </div>
  );
}
