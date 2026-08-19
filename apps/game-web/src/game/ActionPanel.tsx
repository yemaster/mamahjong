import { useState } from "react";
import type {
  GameCommandName,
  MatchView,
  ReactionOption,
  TurnActions,
} from "../types";
import { chiOptions } from "./chiOptions";

const emptyTurnActions: TurnActions = {
  can_tsumo: false,
  riichi_discard_tile_ids: [],
  riichi_discard_hints: [],
  tenpai_discard_hints: [],
  concealed_kan_tile_ids: [],
  added_kan_options: [],
  nuki_tile_ids: [],
  can_nine_terminals: false,
};

interface ActionPanelProps {
  view: MatchView;
  onCommand: (name: GameCommandName, payload?: unknown) => void;
  riichiSelecting: boolean;
  onRiichiSelectingChange: (selecting: boolean) => void;
  /** 正在挑吃哪一组：这一排按钮整个收起，屏幕交给方案框。 */
  chiSelecting?: boolean;
  onChiSelectingChange?: (selecting: boolean) => void;
  skipCalls?: boolean;
  /** 有浮层正在播（例如冲击麻将的杠点），播完之前整排按钮先收起来。 */
  blocked?: boolean;
}

export function ActionPanel({
  view,
  onCommand,
  riichiSelecting,
  onRiichiSelectingChange,
  chiSelecting = false,
  onChiSelectingChange,
  skipCalls = false,
  blocked = false,
}: ActionPanelProps) {
  const [dismissedVersion, setDismissedVersion] = useState<number | null>(null);
  const impact = view.variant_kind === "impact";
  const sichuan = view.variant_kind === "sichuan";
  const allReactions = view.available_reactions ?? [];
  const reactions = skipCalls
    ? allReactions.filter((reaction) => reaction.kind === "ron")
    : allReactions;
  const observerHand =
    view.players.find((player) => player.seat === view.observer_seat)
      ?.concealed_tiles ?? [];
  const chiChoices = chiOptions(reactions, observerHand);
  const actions = view.turn_actions ?? emptyTurnActions;
  const hasResponse = reactions.length > 0;
  /* 可执行动作完全由后端下发，前端只负责展示并提交。 */
  const impactConcealedKanCodes =
    actions.impact_concealed_kan_tile_codes ?? [];
  const impactAddedKanMeldIds = actions.impact_added_kan_meld_ids ?? [];
  const impactIndicatorKan = actions.impact_indicator_concealed_kan ?? false;
  const sichuanConcealedKanCodes =
    actions.sichuan_concealed_kan_tile_codes ?? [];
  const sichuanAddedKanMeldIds = actions.sichuan_added_kan_meld_ids ?? [];
  const hasTurnAction = impact
    ? actions.can_tsumo ||
      impactConcealedKanCodes.length > 0 ||
      impactAddedKanMeldIds.length > 0 ||
      impactIndicatorKan
    : sichuan
      ? actions.can_tsumo ||
        sichuanConcealedKanCodes.length > 0 ||
        sichuanAddedKanMeldIds.length > 0
      : actions.can_tsumo ||
        actions.riichi_discard_tile_ids.length > 0 ||
        actions.concealed_kan_tile_ids.length > 0 ||
        actions.added_kan_options.length > 0 ||
        actions.nuki_tile_ids.length > 0 ||
        actions.can_nine_terminals;

  /* 方案框浮在手牌正上方，这一排按钮再留着只会两块板抢屏。 */
  if (
    view.phase.kind === "ended" ||
    chiSelecting ||
    blocked ||
    (!hasResponse &&
      (!hasTurnAction || dismissedVersion === view.version))
  ) {
    return null;
  }

  return (
    <div className="match-actions" aria-label="对局操作">
      {!hasResponse && impact && (
        <>
          {actions.can_tsumo && (
            <ActionButton
              label="自摸"
              tone="win"
              onClick={() => onCommand("impact.tsumo")}
            />
          )}
          {impactConcealedKanCodes.map((code) => (
            <ActionButton
              key={code}
              label="暗杠"
              tone="kan"
              onClick={() =>
                onCommand("impact.concealed_kan", { tile_code: code })
              }
            />
          ))}
          {impactAddedKanMeldIds.map((meldId) => (
            <ActionButton
              key={meldId}
              label="加杠"
              tone="kan"
              onClick={() =>
                onCommand("impact.added_kan", { meld_id: meldId })
              }
            />
          ))}
          {/*
            指示牌暗杠只结算杠点，牌型仍算刻子、不摸岭上牌，所以按钮上写全名，
            免得和真暗杠混作一谈。
          */}
          {impactIndicatorKan && (
            <ActionButton
              label="指示牌暗杠"
              tone="kan"
              onClick={() => onCommand("impact.indicator_concealed_kan")}
            />
          )}
          <ActionButton
            label="取消"
            tone="quiet"
            onClick={() => setDismissedVersion(view.version)}
          />
        </>
      )}

      {!hasResponse && sichuan && (
        <>
          {actions.can_tsumo && (
            <ActionButton
              label="自摸"
              tone="win"
              onClick={() => onCommand("sichuan.tsumo")}
            />
          )}
          {sichuanConcealedKanCodes.map((code) => (
            <ActionButton
              key={code}
              label="暗杠"
              tone="kan"
              onClick={() =>
                onCommand("sichuan.concealed_kan", { tile_code: code })
              }
            />
          ))}
          {sichuanAddedKanMeldIds.map((meldId) => (
            <ActionButton
              key={meldId}
              label="加杠"
              tone="kan"
              onClick={() => onCommand("sichuan.added_kan", { meld_id: meldId })}
            />
          ))}
          <ActionButton
            label="取消"
            tone="quiet"
            onClick={() => setDismissedVersion(view.version)}
          />
        </>
      )}

      {!hasResponse && !impact && !sichuan && (
        <>
          {riichiSelecting ? (
            <ActionButton
              label="取消"
              tone="quiet"
              onClick={() => onRiichiSelectingChange(false)}
            />
          ) : (
            <>
              {actions.can_tsumo && (
                <ActionButton
                  label="自摸"
                  tone="win"
                  onClick={() => onCommand("riichi.tsumo")}
                />
              )}
              {actions.concealed_kan_tile_ids.length > 0 && (
                <ActionButton
                  label="暗杠"
                  tone="kan"
                  onClick={() =>
                    onCommand("riichi.concealed_kan", {
                      tile_ids: actions.concealed_kan_tile_ids[0],
                    })
                  }
                />
              )}
              {actions.added_kan_options.map((option) => (
                <ActionButton
                  key={option.meld_id}
                  label="加杠"
                  tone="kan"
                  onClick={() =>
                    onCommand("riichi.added_kan", {
                      meld_id: option.meld_id,
                      tile_id: option.tile_id,
                    })
                  }
                />
              ))}
              {actions.nuki_tile_ids.length > 0 && (
                <ActionButton
                  label="拔北"
                  tone="kan"
                  onClick={() =>
                    onCommand("riichi.nuki", {
                      tile_id: actions.nuki_tile_ids[0],
                    })
                  }
                />
              )}
              {/*
                按钮上只写「流局」：按下去的时候玩家要的就是流掉这一局，至于是
                九种九牌还是别的哪一种，等真流了再由横幅在牌桌上写清楚。按钮位置
                窄，写全名既挤又没必要。
              */}
              {actions.can_nine_terminals && (
                <ActionButton
                  label="流局"
                  tone="draw"
                  onClick={() => onCommand("riichi.nine_terminals")}
                />
              )}
              {actions.riichi_discard_tile_ids.length > 0 && (
                <ActionButton
                  label="立直"
                  tone="win"
                  onClick={() => onRiichiSelectingChange(true)}
                />
              )}
              <ActionButton
                label="取消"
                tone="quiet"
                onClick={() => setDismissedVersion(view.version)}
              />
            </>
          )}
        </>
      )}

      {hasResponse && impact && (
        <>
          {reactionOfKind(reactions, "ron") && (
            <ActionButton
              label="荣和"
              tone="win"
              onClick={() => onCommand("impact.ron")}
            />
          )}
          {chiChoices.length > 0 && (
            <ActionButton
              label="吃"
              tone="chi"
              onClick={() => {
                if (chiChoices.length > 1) {
                  onChiSelectingChange?.(true);
                  return;
                }
                onCommand("impact.chi", { tile_ids: chiChoices[0]!.tileIds });
              }}
            />
          )}
          {(() => {
            const pon = reactionOfKind(reactions, "impact_pon");
            if (pon?.kind !== "impact_pon") return null;
            return (
              <ActionButton
                /* 碰的是财神指示牌：按明杠结算杠点，但摆出来仍是刻子。 */
                label={pon.indicator ? "指示牌碰" : "碰"}
                tone="pon"
                onClick={() => onCommand("impact.pon")}
              />
            );
          })()}
          {reactionOfKind(reactions, "impact_open_kan") && (
            <ActionButton
              label="杠"
              tone="kan"
              onClick={() => onCommand("impact.open_kan")}
            />
          )}
          <ActionButton
            label="取消"
            tone="quiet"
            onClick={() => onCommand("impact.pass")}
          />
        </>
      )}

      {hasResponse && sichuan && (
        <>
          {reactions.some((reaction) => reaction.kind === "ron") && (
            <ActionButton
              label="荣和"
              tone="win"
              onClick={() => onCommand("sichuan.ron")}
            />
          )}
          {reactionOfKind(reactions, "sichuan_pon") && (
            <ActionButton
              label="碰"
              tone="pon"
              onClick={() => onCommand("sichuan.pon")}
            />
          )}
          {reactionOfKind(reactions, "sichuan_open_kan") && (
            <ActionButton
              label="杠"
              tone="kan"
              onClick={() => onCommand("sichuan.open_kan")}
            />
          )}
          <ActionButton
            label="取消"
            tone="quiet"
            onClick={() => onCommand("sichuan.pass")}
          />
        </>
      )}

      {hasResponse && !impact && !sichuan && (
        <>
          {reactions.some((reaction) => reaction.kind === "ron") && (
            <ActionButton
              label="荣和"
              tone="win"
              onClick={() => onCommand("riichi.ron")}
            />
          )}
          {reactionOfKind(reactions, "pon") && (
            <ActionButton
              label="碰"
              tone="pon"
              onClick={() => {
                const reaction = reactionOfKind(reactions, "pon");
                if (reaction?.kind === "pon") {
                  onCommand("riichi.pon", {
                    tile_ids: reaction.tile_ids,
                  });
                }
              }}
            />
          )}
          {/*
            吃法不止一种时不替玩家挑：哪一组留在手上是打牌的一部分，按下「吃」
            只是打开方案框。只有一种就直接吃——为一个没得选的选择开一块面板，
            等于多要玩家点一次。
          */}
          {chiChoices.length > 0 && (
            <ActionButton
              label="吃"
              tone="chi"
              onClick={() => {
                if (chiChoices.length > 1) {
                  onChiSelectingChange?.(true);
                  return;
                }
                onCommand("riichi.chi", { tile_ids: chiChoices[0]!.tileIds });
              }}
            />
          )}
          {reactionOfKind(reactions, "open_kan") && (
            <ActionButton
              label="杠"
              tone="kan"
              onClick={() => {
                const reaction = reactionOfKind(reactions, "open_kan");
                if (reaction?.kind === "open_kan") {
                  onCommand("riichi.open_kan", {
                    tile_ids: reaction.tile_ids,
                  });
                }
              }}
            />
          )}
          <ActionButton
            label="取消"
            tone="quiet"
            onClick={() => onCommand("riichi.pass")}
          />
        </>
      )}
    </div>
  );
}

/**
 * 按钮的配色。和了/立直用最扎眼的红，吃碰杠各占一色好分辨，主动流局跟流局横幅
 * 一样压成暗紫，取消退到最淡，其余走默认的金色。
 */
type ActionTone =
  | "win"
  | "chi"
  | "pon"
  | "kan"
  | "draw"
  | "quiet"
  | "plain";

const ACTION_TONE_CLASS: Record<ActionTone, string> = {
  win: "is-emphasis",
  chi: "is-chi",
  pon: "is-pon",
  kan: "is-kan",
  draw: "is-draw",
  quiet: "is-quiet",
  plain: "",
};

function ActionButton({
  label,
  onClick,
  tone = "plain",
  disabled = false,
}: {
  label: string;
  onClick: () => void;
  tone?: ActionTone;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={`match-brush-button ${ACTION_TONE_CLASS[tone]}`.trimEnd()}
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </button>
  );
}

function reactionOfKind(
  reactions: ReactionOption[],
  kind: ReactionOption["kind"],
): ReactionOption | undefined {
  return reactions.find((reaction) => reaction.kind === kind);
}
