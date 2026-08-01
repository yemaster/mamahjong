interface MatchmakingSceneProps {
  ticketId: string;
}

export default function MatchmakingScene({ ticketId }: MatchmakingSceneProps) {
  return (
    <div style={{ padding: 40, fontSize: 16, color: "var(--color-text)" }}>
      匹配中 {ticketId} — 即将实现
    </div>
  );
}
