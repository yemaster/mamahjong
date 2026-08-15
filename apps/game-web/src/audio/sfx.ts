/**
 * 一次性音效（打牌落河、点数变动等）的加载与播放。
 *
 * 和 voice.ts / music.ts 同一套模式：load 一次缓进 Map，之后随时用 playSfx
 * 直接播，不需要再等文件。preloadSfx 返回的 Promise 永远 resolve——网慢或者
 * 文件缺失只是静音，不会把调用方卡住。
 */

import { useAudioSettings } from "../stores/audioSettingsStore";

/** 加载一个 sfx 文件最多等这么久，超时就当 ok，静音继续。 */
const PRELOAD_TIMEOUT_MS = 10_000;

const cache = new Map<string, HTMLAudioElement>();

useAudioSettings.subscribe((current, previous) => {
  if (current.sfxVolume === previous.sfxVolume) return;
  /* 包括已经开始播放的长音效；HTMLAudio.volume 修改后会立即生效。 */
  for (const audio of cache.values()) audio.volume = current.sfxVolume;
});

/** 把音效文件 load 进 cache。同一个 src 多次调用等同于一次。 */
export function preloadSfx(
  src: string,
  timeoutMs = PRELOAD_TIMEOUT_MS,
): Promise<void> {
  if (cache.has(src)) return Promise.resolve();
  return new Promise<void>((resolve) => {
    const audio = new Audio(src);
    cache.set(src, audio);
    const done = () => resolve();
    audio.addEventListener("canplaythrough", done, { once: true });
    audio.addEventListener("error", done, { once: true });
    window.setTimeout(done, timeoutMs);
    audio.load();
  });
}

/**
 * 播放已经 preload 过的音效。
 * src 不在 cache 里（还没 preload 或者 preload 失败）就静默跳过。
 */
export function playSfx(src: string): void {
  const audio = cache.get(src);
  if (!audio) return;
  audio.currentTime = 0;
  audio.volume = useAudioSettings.getState().sfxVolume;
  void audio.play().catch(() => undefined);
}

/**
 * 播放音效并等待播完（或出错/超时）。
 *
 * 结算里番种逐个亮出时需要等前一条音效播完再放下一条，所以不能只 fire-and-forget。
 */
export function playSfxAndWait(
  src: string,
  timeoutMs = 8_000,
): Promise<void> {
  const audio = cache.get(src);
  if (!audio) return Promise.resolve();
  audio.currentTime = 0;
  audio.volume = useAudioSettings.getState().sfxVolume;
  return new Promise<void>((resolve) => {
    const done = () => resolve();
    audio.addEventListener("ended", done, { once: true });
    audio.addEventListener("error", done, { once: true });
    window.setTimeout(done, timeoutMs);
    void audio.play().catch(done);
  });
}

/** 打出的牌飞到牌河时的落地音效。 */
export const DISCARD_SFX = `${import.meta.env.BASE_URL}assets/sfx/discard_tile.mp3`;
/** 点数/杠点变动时连续播 6 次的计分音效（替代原来的 winlose）。 */
export const SCORE_CHANGE_SFX = `${import.meta.env.BASE_URL}assets/sfx/score_change.mp3`;
/** 和了结算每个番种出现时的音效。 */
export const HULE_FAN_OUT_SFX = `${import.meta.env.BASE_URL}assets/sfx/hule_fan_out.mp3`;
/** 立直麻将番符出现时的音效。 */
export const FU_APPEAR_SFX = `${import.meta.env.BASE_URL}assets/sfx/fu_appear.mp3`;
/** 总得点出现时的音效。 */
export const SCORE_APPEAR_SFX = `${import.meta.env.BASE_URL}assets/sfx/score_appear.mp3`;
/** 鼠标/触屏点击音效。 */
export const MOUSECLICK_SFX = `${import.meta.env.BASE_URL}assets/sfx/mouseclick.mp3`;
/** 开局配牌一张张上手时的音效。 */
export const NEWROUND_PAIS_SFX = `${import.meta.env.BASE_URL}assets/sfx/newround_pais.mp3`;
