import type { CSSProperties, ReactNode } from "react";
import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsLeftRight,
  ChevronsRight,
  ChevronsRightLeft,
  Pause,
  Play,
} from "lucide-react";
import { HandPicker, type HandOption } from "./HandPicker";
import { useDragPosition } from "./useDragPosition";

/**
 * 重演的控制条。
 *
 * 不钉在屏幕底边：左端是一条握把，按住能把整条拖到画面任何位置，右端能把它收成
 * 一小块，别挡着想看的那一角。默认摆在主视角手牌上方一点的地方——手最先落在那儿，
 * 又不至于压住自己的牌。
 *
 * 从左到右：跳局、跳巡、单步与自动播放，最后四盏开关灯（牌山、摊牌、铳牌、听牌）。
 * 播放按钮就压在原先写「摸牌/打牌」的那个位置上——那三个字牌桌上本来就看得出来，
 * 位置留给真正要按的东西。
 *
 * 整条挂在 `MatchStage` 里，尺寸一律写固定设计像素——舞台内不许出现 `vw` / `vh` /
 * `clamp()`（见 `docs/game-table-visual-spec.md` 的界面基准分辨率一节）。
 */

export interface ReplayToggles {
  /** 摊开别人的手牌，正面朝上平摊在桌面上。 */
  revealHands: boolean;
  /** 铳牌提示：桌上有人听的牌全部染红。 */
  dangerTiles: boolean;
  /** 听牌提示：听牌那家的角色卡片旁出现一张写着听牌的小卡。 */
  tenpaiHints: boolean;
}

export const DEFAULT_REPLAY_TOGGLES: ReplayToggles = {
  revealHands: false,
  dangerTiles: false,
  tenpaiHints: false,
};

export interface ReplayControlsProps {
  wallOpen: boolean;
  onToggleWall: () => void;
  handOptions: HandOption[];
  handIndex: number;
  onSelectHand: (index: number) => void;
  turnCount: number;
  turnIndex: number;
  onSelectTurn: (turn: number) => void;
  canStepBack: boolean;
  canStepForward: boolean;
  onStepBack: () => void;
  onStepForward: () => void;
  playing: boolean;
  onTogglePlay: () => void;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  toggles: ReplayToggles;
  onTogglesChange: (next: ReplayToggles) => void;
}

/** 条上的图标按钮：只有图标，不套框——整条外面那一圈金边已经够了。 */
function BarButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className="replay-bar__button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
    >
      {children}
    </button>
  );
}

/** 开关灯：亮起来铺一块金板黑字，灭着就是一行灰字，一眼能扫出开了哪几个。 */
function Lamp({
  label,
  on,
  onClick,
}: {
  label: string;
  on: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`replay-bar__lamp${on ? " is-on" : ""}`}
      onClick={onClick}
      aria-pressed={on}
    >
      <i aria-hidden="true" />
      {label}
    </button>
  );
}

export function ReplayControls({
  wallOpen,
  onToggleWall,
  handOptions,
  handIndex,
  onSelectHand,
  turnCount,
  turnIndex,
  onSelectTurn,
  canStepBack,
  canStepForward,
  onStepBack,
  onStepForward,
  playing,
  onTogglePlay,
  collapsed,
  onToggleCollapsed,
  toggles,
  onTogglesChange,
}: ReplayControlsProps) {
  /*
   * 抬到主视角手牌上方：手牌下边距 76px（`--match-hand-bottom`）＋牌高 88px
   * （`--match-hand-tile-height`）＝手牌顶边离舞台底边 164px，再留 44px 空档。
   */
  const { position, nodeRef, handleProps } = useDragPosition(208);

  /* 量出位置之前先藏着：从左上角闪一下再跳过去太难看。 */
  const style: CSSProperties = position
    ? { left: position.x, top: position.y }
    : { left: 0, top: 0, visibility: "hidden" };

  return (
    <div
      className={`replay-bar${collapsed ? " is-collapsed" : ""}`}
      ref={nodeRef as React.RefObject<HTMLDivElement>}
      style={style}
      aria-label="重演控制"
    >
      <span
        className="replay-bar__grip"
        title="拖动"
        aria-hidden="true"
        {...handleProps}
      />

      {!collapsed && (
        <>
          <div className="replay-bar__cell">
            <BarButton
              label="上一局"
              onClick={() => onSelectHand(handIndex - 1)}
              disabled={handIndex <= 0}
            >
              <ChevronsLeft aria-hidden="true" />
            </BarButton>
            <HandPicker
              options={handOptions}
              value={handIndex}
              onSelect={onSelectHand}
            />
            <BarButton
              label="下一局"
              onClick={() => onSelectHand(handIndex + 1)}
              disabled={handIndex >= handOptions.length - 1}
            >
              <ChevronsRight aria-hidden="true" />
            </BarButton>
          </div>

          <span className="replay-bar__seam" aria-hidden="true" />

          <div className="replay-bar__cell" aria-label="跳巡">
            <BarButton
              label="上一巡"
              onClick={() => onSelectTurn(turnIndex - 1)}
              disabled={turnIndex <= 1}
            >
              <ChevronLeft aria-hidden="true" />
            </BarButton>
            <span className="replay-bar__turn">
              {turnIndex}
              <i>巡</i>
            </span>
            <BarButton
              label="下一巡"
              onClick={() => onSelectTurn(turnIndex + 1)}
              disabled={turnIndex >= turnCount}
            >
              <ChevronRight aria-hidden="true" />
            </BarButton>
          </div>

          <span className="replay-bar__seam" aria-hidden="true" />

          <div className="replay-bar__cell" aria-label="单步">
            <BarButton
              label="上一步"
              onClick={onStepBack}
              disabled={!canStepBack}
            >
              <ChevronLeft aria-hidden="true" />
            </BarButton>
            {/* 原先这个位置写的是「摸牌／打牌」，换成播放键。 */}
            <button
              type="button"
              className={`replay-bar__play${playing ? " is-on" : ""}`}
              onClick={onTogglePlay}
              aria-label={playing ? "暂停" : "自动播放"}
              title={playing ? "暂停" : "自动播放"}
            >
              {playing ? (
                <Pause aria-hidden="true" />
              ) : (
                <Play aria-hidden="true" />
              )}
            </button>
            <BarButton
              label="下一步"
              onClick={onStepForward}
              disabled={!canStepForward}
            >
              <ChevronRight aria-hidden="true" />
            </BarButton>
          </div>

          <span className="replay-bar__seam" aria-hidden="true" />

          <div className="replay-bar__lamps">
            <Lamp label="牌山" on={wallOpen} onClick={onToggleWall} />
            <Lamp
              label="摊牌"
              on={toggles.revealHands}
              onClick={() =>
                onTogglesChange({
                  ...toggles,
                  revealHands: !toggles.revealHands,
                })
              }
            />
            <Lamp
              label="铳牌"
              on={toggles.dangerTiles}
              onClick={() =>
                onTogglesChange({
                  ...toggles,
                  dangerTiles: !toggles.dangerTiles,
                })
              }
            />
            <Lamp
              label="听牌"
              on={toggles.tenpaiHints}
              onClick={() =>
                onTogglesChange({
                  ...toggles,
                  tenpaiHints: !toggles.tenpaiHints,
                })
              }
            />
          </div>
        </>
      )}

      <button
        type="button"
        className="replay-bar__fold"
        onClick={onToggleCollapsed}
        aria-label={collapsed ? "展开控制条" : "收起控制条"}
      >
        {/* 只给图标：整条是横向伸缩的，箭头往两边开＝展开，往中间合＝收起。 */}
        {collapsed ? (
          <ChevronsLeftRight aria-hidden="true" />
        ) : (
          <ChevronsRightLeft aria-hidden="true" />
        )}
      </button>
    </div>
  );
}
