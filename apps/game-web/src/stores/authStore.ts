import { create } from "zustand";
import type { UserView } from "../types";

const TOKEN_KEY = "mamahjong_token";

function loadToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

function saveToken(token: string | null): void {
  try {
    if (token) {
      localStorage.setItem(TOKEN_KEY, token);
    } else {
      localStorage.removeItem(TOKEN_KEY);
    }
  } catch {
    /* localStorage unavailable — token stays in memory */
  }
}

export interface AuthState {
  token: string | null;
  identity: UserView | null;
  isAuthenticated: boolean;

  setToken: (token: string) => void;
  setIdentity: (identity: UserView) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  token: loadToken(),
  identity: null,
  isAuthenticated: false,

  setToken: (token: string) => {
    saveToken(token);
    set({ token, isAuthenticated: true });
  },

  setIdentity: (identity: UserView) => {
    set({ identity });
  },

  logout: () => {
    saveToken(null);
    set({ token: null, identity: null, isAuthenticated: false });
  },
}));
