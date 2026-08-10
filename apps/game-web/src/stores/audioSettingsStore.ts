import { create } from "zustand";

const STORAGE_KEY = "mamahjong-audio-settings";

export interface AudioSettingsState {
  /** 背景音乐音量 0..1，默认 1（100%）。 */
  musicVolume: number;
  /** 角色语音音量 0..1，默认 1（100%）。 */
  voiceVolume: number;
  /** 打牌音效音量 0..1，默认 1（100%）。 */
  sfxVolume: number;
  setMusicVolume: (v: number) => void;
  setVoiceVolume: (v: number) => void;
  setSfxVolume: (v: number) => void;
}

function load(): { musicVolume: number; voiceVolume: number; sfxVolume: number } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        musicVolume: clamp(parsed.musicVolume, 0, 1, 1),
        voiceVolume: clamp(parsed.voiceVolume, 0, 1, 1),
        sfxVolume: clamp(parsed.sfxVolume, 0, 1, 1),
      };
    }
  } catch {
    /* ignore corrupt data */
  }
  return { musicVolume: 1, voiceVolume: 1, sfxVolume: 1 };
}

function clamp(v: unknown, lo: number, hi: number, fallback: number): number {
  if (typeof v !== "number" || Number.isNaN(v)) return fallback;
  return Math.max(lo, Math.min(hi, v));
}

function persist(s: AudioSettingsState) {
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        musicVolume: s.musicVolume,
        voiceVolume: s.voiceVolume,
        sfxVolume: s.sfxVolume,
      }),
    );
  } catch {
    /* ignore */
  }
}

export const useAudioSettings = create<AudioSettingsState>((set) => ({
  ...load(),
  setMusicVolume: (musicVolume) =>
    set((s) => {
      persist({ ...s, musicVolume });
      return { musicVolume };
    }),
  setVoiceVolume: (voiceVolume) =>
    set((s) => {
      persist({ ...s, voiceVolume });
      return { voiceVolume };
    }),
  setSfxVolume: (sfxVolume) =>
    set((s) => {
      persist({ ...s, sfxVolume });
      return { sfxVolume };
    }),
}));
