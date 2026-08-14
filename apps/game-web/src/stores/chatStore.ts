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
  receive: (seat: number, type: "text" | "emoji", content: string) => void;
  expire: (id: string) => void;
  clear: () => void;
}

let nextId = 0;

export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  receive: (seat, type, content) => {
    const now = Date.now();
    const id = `chat-${++nextId}`;
    set((s) => ({
      messages: [...s.messages, { id, seat, type, content, at: now }],
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
  clear: () => set({ messages: [] }),
}));
