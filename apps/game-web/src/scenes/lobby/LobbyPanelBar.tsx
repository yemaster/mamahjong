import { ArrowLeft } from "lucide-react";

export function LobbyPanelBar({
  title,
  onBack,
}: {
  title: string;
  onBack: () => void;
}) {
  return (
    <header className="game-lobby__panel-bar">
      <button type="button" onClick={onBack} aria-label={`返回${title}上一级`}>
        <ArrowLeft aria-hidden="true" />
        <span>返回</span>
      </button>
      <h2>{title}</h2>
      <span aria-hidden="true" />
    </header>
  );
}
