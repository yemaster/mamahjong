import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { ChevronRight, SendHorizonal, Smile } from "lucide-react";
import { visualPixelsToStage } from "../components/fixedDomStageLayout";
import type { LobbyCharacter, MatchView } from "../types";
import { useChatStore } from "../stores/chatStore";
import { tableRelativeSeat } from "./table";

/* ── ChatBox (输入区) ──────────────────────────── */

interface ChatBoxProps {
  observerSeat: number;
  playerCharacterId: string | null;
  charactersById: Map<string, LobbyCharacter>;
}

const COOLDOWN_MS = 8_000;

export function ChatBox({
  observerSeat,
  playerCharacterId,
  charactersById,
}: ChatBoxProps) {
  const send = useChatStore((s) => s.send);
  const lastSentAt = useChatStore((s) => s.lastSentAt);
  const [text, setText] = useState("");
  const [emojiOpen, setEmojiOpen] = useState(false);
  const [focused, setFocused] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const dragRef = useRef<{
    pointerId: number;
    x: number;
    y: number;
    ox: number;
    oy: number;
    scaleX: number;
    scaleY: number;
  } | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [cooldownLeft, setCooldownLeft] = useState(0);
  /* 表情框超出屏幕时翻转到下方 */
  const [popoverFlip, setPopoverFlip] = useState(false);

  /* 冷却倒计时 */
  useEffect(() => {
    if (cooldownLeft <= 0) return;
    const timer = window.setInterval(() => {
      const elapsed = Date.now() - lastSentAt;
      const left = Math.max(0, COOLDOWN_MS - elapsed);
      setCooldownLeft(left);
      if (left <= 0) window.clearInterval(timer);
    }, 200);
    return () => window.clearInterval(timer);
  }, [cooldownLeft, lastSentAt]);

  const canSend =
    text.trim().length > 0 && cooldownLeft <= 0;

  const doSend = useCallback(() => {
    const trimmed = text.trim();
    if (trimmed.length === 0 || cooldownLeft > 0) return;
    send(observerSeat, "text", trimmed);
    setText("");
    setCooldownLeft(COOLDOWN_MS);
    inputRef.current?.blur();
  }, [text, cooldownLeft, send, observerSeat]);

  const onEmojiPick = useCallback(
    (path: string) => {
      if (cooldownLeft > 0) return;
      send(observerSeat, "emoji", path);
      setEmojiOpen(false);
      setCooldownLeft(COOLDOWN_MS);
    },
    [cooldownLeft, send, observerSeat],
  );

  /* 展开表情框时检查是否超出屏幕上沿 */
  const toggleEmoji = useCallback(() => {
    setEmojiOpen((v) => {
      if (v) return false;
      /* 用 requestAnimationFrame 等 popover ref 挂上再量 */
      requestAnimationFrame(() => {
        const popover = popoverRef.current;
        if (!popover) return;
        const rect = popover.getBoundingClientRect();
        const stageTop =
          popover.closest<HTMLElement>(".fixed-dom-stage__content")
            ?.getBoundingClientRect().top ?? 0;
        if (rect.top < stageTop) {
          setPopoverFlip(true);
        } else {
          setPopoverFlip(false);
        }
      });
      return true;
    });
  }, []);

  /* 拖动 */
  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      const box = boxRef.current;
      const stage = box?.offsetParent as HTMLElement | null;
      if (!box || !stage || e.button !== 0) return;
      const boxRect = box.getBoundingClientRect();
      const stageRect = stage.getBoundingClientRect();
      const scaleX =
        stage.offsetWidth > 0 ? stageRect.width / stage.offsetWidth : 1;
      const scaleY =
        stage.offsetHeight > 0 ? stageRect.height / stage.offsetHeight : 1;
      dragRef.current = {
        pointerId: e.pointerId,
        x: e.clientX,
        y: e.clientY,
        /* client 坐标是缩放后的屏幕像素，left/top 是 1600×900 设计像素。 */
        ox: visualPixelsToStage(boxRect.left - stageRect.left, scaleX),
        oy: visualPixelsToStage(boxRect.top - stageRect.top, scaleY),
        scaleX: Math.max(scaleX, 0.0001),
        scaleY: Math.max(scaleY, 0.0001),
      };
      e.currentTarget.setPointerCapture(e.pointerId);
      e.preventDefault();
    },
    [],
  );

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    const drag = dragRef.current;
    const box = boxRef.current;
    const stage = box?.offsetParent as HTMLElement | null;
    if (!drag || drag.pointerId !== e.pointerId || !box || !stage) return;
    const dx = visualPixelsToStage(e.clientX - drag.x, drag.scaleX);
    const dy = visualPixelsToStage(e.clientY - drag.y, drag.scaleY);
    const maxX = Math.max(0, stage.offsetWidth - box.offsetWidth);
    const maxY = Math.max(0, stage.offsetHeight - box.offsetHeight);
    setPos({
      x: clamp(drag.ox + dx, 0, maxX),
      y: clamp(drag.oy + dy, 0, maxY),
    });
  }, []);

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (dragRef.current?.pointerId !== e.pointerId) return;
    dragRef.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  }, []);

  /* 双击拖动把手折叠/展开 */
  const onDoubleClick = useCallback(() => {
    setCollapsed((v) => !v);
    if (!collapsed) {
      setEmojiOpen(false);
    }
  }, [collapsed]);

  /* 获取当前角色的表情列表 */
  const emotes = playerCharacterId
    ? charactersById.get(playerCharacterId)?.emotes ?? []
    : [];

  const style: React.CSSProperties = pos
    ? { left: `${pos.x}px`, top: `${pos.y}px`, right: "auto", bottom: "auto" }
    : {};

  return (
    <div
      ref={boxRef}
      className={`match-chat-box${collapsed ? " is-collapsed" : ""}`}
      style={style}
    >
      {/* 表情选择框 — 在输入框上方显示 */}
      {emojiOpen && emotes.length > 0 && (
        <div
          ref={popoverRef}
          className={`match-chat-emoji-popover${popoverFlip ? " is-flipped" : ""}`}
        >
          <div className="match-chat-emoji-grid">
            {emotes.map((emote) => (
              <button
                key={emote.path}
                type="button"
                className="match-chat-emoji-item"
                onClick={() => onEmojiPick(emote.path)}
                disabled={cooldownLeft > 0}
              >
                <img src={emote.path} alt={emote.name} />
              </button>
            ))}
          </div>
        </div>
      )}

      {/* 输入区 */}
      <div className="match-chat-input-row">
        {/* 拖动把手 — 左侧 */}
        <div
          className="match-chat-drag-handle"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onDoubleClick={onDoubleClick}
          title="双击折叠"
        />

        {!collapsed && (
          <>
            <input
              ref={inputRef}
              type="text"
              className="match-chat-input"
              placeholder={cooldownLeft > 0 ? `${Math.ceil(cooldownLeft / 1000)}s` : "点击输入文字……"}
              value={text}
              onChange={(e) => setText(e.target.value)}
              onFocus={() => {
                setFocused(true);
                setEmojiOpen(false);
              }}
              onBlur={() => setFocused(false)}
              onKeyDown={(e) => {
                if (e.key === "Enter") doSend();
              }}
              disabled={cooldownLeft > 0}
            />

            {focused ? (
              <button
                type="button"
                className="match-chat-send-btn"
                onClick={doSend}
                disabled={!canSend}
                aria-label="发送"
              >
                <SendHorizonal aria-hidden="true" />
              </button>
            ) : (
              <button
                type="button"
                className="match-chat-emoji-toggle"
                onClick={toggleEmoji}
                aria-label="表情"
                disabled={cooldownLeft > 0 || emotes.length === 0}
              >
                <Smile aria-hidden="true" />
              </button>
            )}
          </>
        )}
        {collapsed && (
          <div className="match-chat-collapsed-hint">
            <ChevronRight aria-hidden="true" />
          </div>
        )}
      </div>
    </div>
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/* ── ChatMessages (气泡展示) ───────────────────── */

/**
 * 在角色卡片附近显示聊天气泡。
 * 放在 MatchStage 里面渲染，位置跟玩家面板走。
 */
export function ChatMessages({ view }: { view: MatchView }) {
  const messages = useChatStore((s) => s.messages);

  /* 按座位把消息分组，每个座位只显示最新一条 */
  const latestBySeat = new Map<number, typeof messages[0]>();
  for (const msg of messages) {
    const existing = latestBySeat.get(msg.seat);
    if (!existing || msg.at > existing.at) {
      latestBySeat.set(msg.seat, msg);
    }
  }

  return (
    <div className="match-chat-bubbles" aria-label="聊天消息">
      {[...latestBySeat.values()].map((msg) => {
        const relative =
          tableRelativeSeat(msg.seat, view.observer_seat, view.players.length);
        const seatClass = `is-seat-${relative}`;
        return (
          <div
            key={msg.id}
            className={`match-chat-bubble ${seatClass}`}
          >
            {msg.type === "emoji" ? (
              <img
                className="match-chat-bubble-emoji"
                src={msg.content}
                alt=""
              />
            ) : (
              <span className="match-chat-bubble-text">{msg.content}</span>
            )}
          </div>
        );
      })}
    </div>
  );
}
