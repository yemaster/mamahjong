import { create } from "zustand";

export interface ChatMessage {
  id: string;
  seat: number;
  type: "text" | "emoji";
  content: string;
  at: number;
}

interface ChatState {
  messages: ChatMessage[];
  lastSentAt: number;
  send: (seat: number, type: "text" | "emoji", content: string) => void;
  expire: (id: string) => void;
}

let nextId = 0;

export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  lastSentAt: 0,
  send: (seat, type, content) => {
    const now = Date.now();
    const id = `chat-${++nextId}`;
    set((s) => ({
      messages: [...s.messages, { id, seat, type, content, at: now }],
      lastSentAt: now,
    }));
    /* 5s 后自动消失 */
    window.setTimeout(() => {
      useChatStore.getState().expire(id);
    }, 5_000);
  },
  expire: (id) => {
    set((s) => ({
      messages: s.messages.filter((m) => m.id !== id),
    }));
  },
}));
