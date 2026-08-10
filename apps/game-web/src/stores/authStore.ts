import { create } from "zustand";
import type { UserView } from "../types";

export const TOKEN_KEY = "mamahjong_token";
const CREDENTIALS_KEY = "mamahjong_credentials";
const KEY_STORE_KEY = "mamahjong_ak";
export const RETURN_TO_SPLASH_EVENT = "mamahjong:return-to-splash";

export interface SavedCredentials {
  loginName: string;
  password: string;
}

export function returnToSplashForLogin(): void {
  window.dispatchEvent(new CustomEvent(RETURN_TO_SPLASH_EVENT));
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

// ── AES‑GCM encryption for stored credentials ────────────────────
//
// Each username gets its own random AES‑256 key, generated once and
// persisted in localStorage.  Decryption failures are treated as
// "no saved credentials" — no error is surfaced to the user.

/** Returns (and persists) the AES key for `loginName`. */
async function getUserKey(loginName: string): Promise<CryptoKey | null> {
  try {
    const raw = localStorage.getItem(KEY_STORE_KEY);
    const store: Record<string, string> = raw ? JSON.parse(raw) : {};
    const exported = store[loginName];
    if (exported) {
      return await crypto.subtle.importKey(
        "raw",
        Uint8Array.from(atob(exported), (c) => c.charCodeAt(0)),
        { name: "AES-GCM" },
        false,
        ["encrypt", "decrypt"],
      );
    }
    // No key yet — generate a fresh one and persist it.
    const key = await crypto.subtle.generateKey(
      { name: "AES-GCM", length: 256 },
      true,
      ["encrypt", "decrypt"],
    );
    const rawKey = new Uint8Array(
      await crypto.subtle.exportKey("raw", key),
    );
    store[loginName] = btoa(String.fromCharCode(...rawKey));
    localStorage.setItem(KEY_STORE_KEY, JSON.stringify(store));
    return key;
  } catch {
    return null;
  }
}

async function encryptPassword(
  loginName: string,
  password: string,
): Promise<string | null> {
  try {
    const key = await getUserKey(loginName);
    if (!key) return null;
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const plaintext = new TextEncoder().encode(password);
    const ciphertext = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv },
      key,
      plaintext,
    );
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return btoa(String.fromCharCode(...combined));
  } catch {
    return null;
  }
}

async function decryptPassword(
  loginName: string,
  encrypted: string,
): Promise<string | null> {
  try {
    const key = await getUserKey(loginName);
    if (!key) return null;
    const combined = Uint8Array.from(atob(encrypted), (c) => c.charCodeAt(0));
    const iv = combined.slice(0, 12);
    const ciphertext = combined.slice(12);
    const decrypted = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv },
      key,
      ciphertext,
    );
    return new TextDecoder().decode(decrypted);
  } catch {
    return null;
  }
}

export async function loadCredentials(): Promise<SavedCredentials | null> {
  try {
    const raw = localStorage.getItem(CREDENTIALS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { loginName: string; password: string };
    if (!parsed.loginName || !parsed.password) return null;
    const password = await decryptPassword(parsed.loginName, parsed.password);
    if (!password) return null;
    return { loginName: parsed.loginName, password };
  } catch {
    return null;
  }
}

export function saveCredentials(loginName: string, password: string): void {
  encryptPassword(loginName, password)
    .then((encrypted) => {
      if (!encrypted) return;
      try {
        localStorage.setItem(
          CREDENTIALS_KEY,
          JSON.stringify({ loginName, password: encrypted }),
        );
      } catch {
        /* localStorage unavailable */
      }
    })
    .catch(() => {
      /* Crypto unavailable — credentials won't be persisted. */
    });
}

export function clearCredentials(): void {
  try {
    localStorage.removeItem(CREDENTIALS_KEY);
  } catch {
    /* localStorage unavailable */
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
  // Never reuse a saved token — every tab must go through login so the
  // server can revoke the previous session.
  token: null,
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
