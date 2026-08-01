interface RoomSceneProps {
  roomId: string;
}

export default function RoomScene({ roomId }: RoomSceneProps) {
  return (
    <div style={{ padding: 40, fontSize: 16, color: "var(--color-text)" }}>
      房间 {roomId}
    </div>
  );
}
