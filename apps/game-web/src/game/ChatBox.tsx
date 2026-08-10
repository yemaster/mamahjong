import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { ChevronRight, SendHorizonal, Smile } from "lucide-react";
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
  const dragRef = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);
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
        if (rect.top < 0) {
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
      if (!box) return;
      const rect = box.getBoundingClientRect();
      dragRef.current = {
        x: e.clientX,
        y: e.clientY,
        ox: rect.left,
        oy: rect.top,
      };
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [],
  );

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragRef.current) return;
    const dx = e.clientX - dragRef.current.x;
    const dy = e.clientY - dragRef.current.y;
    setPos({ x: dragRef.current.ox + dx, y: dragRef.current.oy + dy });
  }, []);

  const onPointerUp = useCallback(() => {
    dragRef.current = null;
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
