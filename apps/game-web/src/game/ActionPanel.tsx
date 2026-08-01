import { Button } from "../components/Button";
import type { GameCommandName, MatchView, ReactionOption } from "../types";

const panel: React.CSSProperties = {
  position: "absolute",
  bottom: 48,
  left: "50%",
  transform: "translateX(-50%)",
  display: "flex",
  gap: 8,
  padding: "8px 16px",
  background: "rgba(0,0,0,0.6)",
  borderRadius: "var(--radius)",
  zIndex: 10,
};

interface ActionPanelProps {
  view: MatchView;
  onCommand: (name: GameCommandName, payload?: unknown) => void;
  selectedTileId?: number;
}

export function ActionPanel({ view, onCommand, selectedTileId }: ActionPanelProps) {
  const hasResponse = view.available_reactions.length > 0;
  const phaseKind = view.phase.kind;

  if (phaseKind === "ended") return null;

  return (
    <div style={panel}>
      {/* Turn actions (discard / special actions) */}
      {!hasResponse && (
        <>
          <Button
            size="sm"
            onClick={() => {
              if (selectedTileId != null) {
                const isRiichi =
                  view.turn_actions.riichi_discard_tile_ids.includes(
                    selectedTileId,
                  );
                onCommand(
                  isRiichi ? "riichi.riichi_discard" : "riichi.discard",
                  { tile_id: selectedTileId },
                );
              }
            }}
            disabled={selectedTileId == null}
          >
            打牌
          </Button>
          {view.turn_actions.can_tsumo && (
            <Button size="sm" onClick={() => onCommand("riichi.tsumo")}>
              自摸
            </Button>
          )}
          {view.turn_actions.concealed_kan_tile_ids.length > 0 && (
            <Button
              size="sm"
              onClick={() =>
                onCommand("riichi.concealed_kan", {
                  tile_ids: view.turn_actions.concealed_kan_tile_ids[0],
                })
              }
            >
              暗杠
            </Button>
          )}
          {view.turn_actions.added_kan_options.map((opt) => (
            <Button
              key={opt.meld_id}
              size="sm"
              onClick={() =>
                onCommand("riichi.added_kan", {
                  meld_id: opt.meld_id,
                  tile_id: opt.tile_id,
                })
              }
            >
              加杠
            </Button>
          ))}
          {view.turn_actions.can_nine_terminals && (
            <Button
              size="sm"
              onClick={() => onCommand("riichi.nine_terminals")}
            >
              九种九牌
            </Button>
          )}
        </>
      )}

      {/* Response actions */}
      {hasResponse && (
        <>
          {view.available_reactions.some((r) => r.kind === "ron") && (
            <Button size="sm" onClick={() => onCommand("riichi.ron")}>
              荣和
            </Button>
          )}
          {hasReaction(view.available_reactions, "pon") && (
            <Button
              size="sm"
              onClick={() => {
                const pon = view.available_reactions.find(
                  (r) => r.kind === "pon",
                ) as { kind: "pon"; tile_ids: [number, number] } | undefined;
                if (pon) {
                  onCommand("riichi.pon", { tile_ids: pon.tile_ids });
                }
              }}
            >
              碰
            </Button>
          )}
          {hasReaction(view.available_reactions, "chi") && (
            <Button
              size="sm"
              onClick={() => {
                const chi = view.available_reactions.find(
                  (r) => r.kind === "chi",
                ) as { kind: "chi"; tile_ids: [number, number] } | undefined;
                if (chi) {
                  onCommand("riichi.chi", { tile_ids: chi.tile_ids });
                }
              }}
            >
              吃
            </Button>
          )}
          {hasReaction(view.available_reactions, "open_kan") && (
            <Button
              size="sm"
              onClick={() => {
                const kan = view.available_reactions.find(
                  (r) => r.kind === "open_kan",
                ) as
                  | { kind: "open_kan"; tile_ids: [number, number, number] }
                  | undefined;
                if (kan) {
                  onCommand("riichi.open_kan", { tile_ids: kan.tile_ids });
                }
              }}
            >
              杠
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onCommand("riichi.pass")}
          >
            过
          </Button>
        </>
      )}
    </div>
  );
}

function hasReaction(
  reactions: ReactionOption[],
  kind: ReactionOption["kind"],
): boolean {
  return reactions.some((r) => r.kind === kind);
}
