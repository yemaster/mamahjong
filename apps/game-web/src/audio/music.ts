import type { MusicScene, MusicTrackView } from "../types";
import { useAudioSettings } from "../stores/audioSettingsStore";

/** 背景音乐基础音量。压得比音效低，牌桌上的声音要盖得住它。 */
const BASE_MUSIC_VOLUME = 0.45;

function musicVolume(): number {
  return BASE_MUSIC_VOLUME * useAudioSettings.getState().musicVolume;
}

/** 试听的基础音量；仍然受玩家的背景音乐音量设置控制。 */
const PREVIEW_VOLUME = 0.7;
function previewVolume(): number {
  return PREVIEW_VOLUME * useAudioSettings.getState().musicVolume;
}
/** 换曲淡入淡出的时长。 */
const FADE_MS = 700;
/** 淡入淡出的步进间隔。 */
const FADE_STEP_MS = 50;
/** 素材没进 git，缺文件时不能把玩家卡在加载里，到点就当load完了。 */
const PRELOAD_TIMEOUT_MS = 20_000;

const cache = new Map<string, HTMLAudioElement>();

let background: HTMLAudioElement | null = null;
let backgroundSrc: string | null = null;
let preview: HTMLAudioElement | null = null;
/** 试听期间背景音乐是不是被按下去了，试听结束要放回来。 */
let backgroundDucked = false;
/** 上一次 play() 被浏览器挡了，等下一次用户点按再试。 */
let pendingRetry = false;
/** 正在播放的立直音乐；不是立直状态就是 null。 */
let riichiMusic: HTMLAudioElement | null = null;
let riichiMusicSrc: string | null = null;

/** 每个音频最多只有一条淡入淡出，实时拖音量时先结束旧淡变，避免下一拍覆盖滑块。 */
const fadeTimers = new WeakMap<HTMLAudioElement, number>();

const audioAvailable = () => typeof Audio !== "undefined";

/* 浏览器经常不给自动播：进来的路上如果没有用户手势，play() 就会 reject。
   对局和大厅都靠 splash 那一下点按拿到手势，但如果曲库还没 load 完、
   App 还没挂上 effect，点按那一下发出去的 play 就只够放 lobby 的，对局曲
   等 catalog 到了才切——这时候手势已经过了。

   这里记一笔，下一次用户点按（任意位置）帮它补一刀。 */
function armRetryOnInteraction(): void {
  if (pendingRetry) return;
  pendingRetry = true;
  const retry = () => {
    pendingRetry = false;
    document.removeEventListener("pointerdown", retry);
    document.removeEventListener("keydown", retry);
    const current = background;
    if (current && backgroundSrc) {
      current.pause();
      current.currentTime = 0;
      void current.play().then(
        () => fade(current, backgroundDucked ? 0 : musicVolume()),
        () => { backgroundSrc = null; },
      );
    }
  };
  document.addEventListener("pointerdown", retry, { once: true });
  document.addEventListener("keydown", retry, { once: true });
}

function element(src: string): HTMLAudioElement {
  const cached = cache.get(src);
  if (cached) {
    return cached;
  }
  const audio = new Audio(src);
  audio.preload = "auto";
  audio.volume = musicVolume();
  cache.set(src, audio);
  return audio;
}

function fade(audio: HTMLAudioElement, to: number, done?: () => void): void {
  cancelFade(audio);
  const from = audio.volume;
  const steps = Math.max(1, Math.round(FADE_MS / FADE_STEP_MS));
  let step = 0;
  const timer = window.setInterval(() => {
    step += 1;
    const ratio = Math.min(1, step / steps);
    audio.volume = Math.min(1, Math.max(0, from + (to - from) * ratio));
    if (ratio >= 1) {
      window.clearInterval(timer);
      if (fadeTimers.get(audio) === timer) fadeTimers.delete(audio);
      done?.();
    }
  }, FADE_STEP_MS);
  fadeTimers.set(audio, timer);
}

function cancelFade(audio: HTMLAudioElement): void {
  const timer = fadeTimers.get(audio);
  if (timer == null) return;
  window.clearInterval(timer);
  fadeTimers.delete(audio);
}

/** 把新的设置立即应用到当前正在响的实例，不等切歌或下一次 play。 */
function syncPlayingMusicVolume(): void {
  if (preview) {
    cancelFade(preview);
    preview.volume = previewVolume();
  }
  if (riichiMusic && riichiMusic !== preview) {
    cancelFade(riichiMusic);
    riichiMusic.volume = musicVolume();
  }
  if (
    background &&
    background !== preview &&
    background !== riichiMusic
  ) {
    cancelFade(background);
    background.volume = backgroundDucked || riichiMusic ? 0 : musicVolume();
  }
}

useAudioSettings.subscribe((current, previous) => {
  if (current.musicVolume !== previous.musicVolume) {
    syncPlayingMusicVolume();
  }
});

/**
 * 把一首曲子load到能连续播放为止。
 *
 * 永远 resolve：素材缺失、解码失败、网络太慢都算「load完」，不然进不去大厅也
 * 进不去对局。真正的等待由调用方的超时兜底。
 */
export function preloadMusic(
  src: string | null,
  timeoutMs: number = PRELOAD_TIMEOUT_MS,
): Promise<void> {
  if (!src || !audioAvailable()) {
    return Promise.resolve();
  }
  const audio = element(src);
  if (audio.readyState >= 4) {
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) {
        return;
      }
      settled = true;
      window.clearTimeout(timer);
      audio.removeEventListener("canplaythrough", finish);
      audio.removeEventListener("error", finish);
      resolve();
    };
    const timer = window.setTimeout(finish, timeoutMs);
    audio.addEventListener("canplaythrough", finish);
    audio.addEventListener("error", finish);
    audio.load();
  });
}

/** 循环播放背景音乐，同一首不打断。传 null 就停掉。 */
export function playMusic(src: string | null): void {
  if (!audioAvailable() || backgroundSrc === src) {
    return;
  }
  const previous = background;
  backgroundSrc = src;
  if (previous) {
    fade(previous, 0, () => {
      previous.pause();
      previous.currentTime = 0;
    });
  }
  if (!src) {
    background = null;
    return;
  }
  const audio = element(src);
  /* 要播的这首正在试听，先把试听掐掉，不然 loop/volume 会被试听和播放逻辑来回改。 */
  if (preview === audio) {
    stopPreview();
  }
  audio.loop = true;
  audio.volume = 0;
  background = audio;
  if (preview && preview !== audio) {
    // 正在试听别的曲子，换好的曲子先搁着，试听结束再响。
    backgroundDucked = true;
    return;
  }
  void audio.play().then(
    () => fade(audio, backgroundDucked ? 0 : musicVolume()),
    // 浏览器还没拿到用户手势，等下一次点按再试。
    () => {
      armRetryOnInteraction();
    },
  );
}

/** 停掉背景音乐。 */
export function stopMusic(): void {
  playMusic(null);
}

/**
 * 试听一首曲子，load完并开始播放后 resolve。
 *
 * 试听期间背景音乐先按下去，放完或手动停止再放回来。
 */
export async function previewMusic(
  src: string,
  onEnded?: () => void,
): Promise<void> {
  if (!audioAvailable()) {
    return;
  }
  stopPreview();
  await preloadMusic(src);
  const audio = element(src);
  audio.loop = false;
  audio.currentTime = 0;
  audio.volume = previewVolume();
  preview = audio;
  if (background && background !== audio) {
    backgroundDucked = true;
    background.pause();
  }
  const ended = () => {
    audio.removeEventListener("ended", ended);
    if (preview === audio) {
      preview = null;
      restoreBackground();
    }
    onEnded?.();
  };
  audio.addEventListener("ended", ended);
  try {
    await audio.play();
  } catch {
    ended();
  }
}

/** 停掉试听，把背景音乐放回来。 */
export function stopPreview(): void {
  const audio = preview;
  preview = null;
  if (audio) {
    audio.pause();
    audio.currentTime = 0;
  }
  restoreBackground();
}

function restoreBackground(): void {
  if (!backgroundDucked) {
    return;
  }
  backgroundDucked = false;
  if (background) {
    background.volume = musicVolume();
    void background.play().catch(() => undefined);
  }
}

/**
 * 有人立直时把对局音乐换成该玩家的立直曲目。
 *
 * 同一首不打断；传 null 就停在当前立直曲（不会自己停）。
 * 立直曲也走循环，和普通背景音乐同一套音量。
 */
export function playRiichiMusic(src: string | null): void {
  if (!audioAvailable() || riichiMusicSrc === src) {
    return;
  }
  const previousRiichi = riichiMusic;
  riichiMusicSrc = src;
  if (previousRiichi) {
    fade(previousRiichi, 0, () => {
      previousRiichi.pause();
      previousRiichi.currentTime = 0;
    });
  }
  /* 把对局背景按下去（不设 null，停掉再立直结束时才能按原 src 复原）。 */
  if (background) {
    fade(background, 0, () => {
      background?.pause();
    });
  }
  if (!src) {
    riichiMusic = null;
    return;
  }
  const audio = element(src);
  audio.loop = true;
  audio.volume = 0;
  riichiMusic = audio;
  void audio.play().then(
    () => fade(audio, musicVolume()),
    () => {
      armRetryOnInteraction();
    },
  );
}

/**
 * 立直结束（一局打完），淡出立直曲，恢复原来的对局背景音乐。
 *
 * 只关立直那一首，不动别的。
 */
export function stopRiichiMusic(): void {
  if (!riichiMusic) return;
  const audio = riichiMusic;
  riichiMusic = null;
  riichiMusicSrc = null;
  fade(audio, 0, () => {
    audio.pause();
    audio.currentTime = 0;
  });
  /* 把对局背景放回来。 */
  if (background && backgroundSrc) {
    const bg = background;
    bg.volume = 0;
    bg.currentTime = 0;
    void bg.play().then(
      () => fade(bg, musicVolume()),
      () => {
        armRetryOnInteraction();
      },
    );
  }
}

/** 玩家选的那一首；没选或选的曲子没了就退回该场景的默认曲。 */
export function resolveTrack(
  tracks: MusicTrackView[] | undefined,
  scene: MusicScene,
  selectedId: string | null | undefined,
): MusicTrackView | null {
  if (!tracks || tracks.length === 0) {
    return null;
  }
  const inScene = tracks.filter(
    (track) => track.scene === scene && track.enabled,
  );
  return (
    inScene.find((track) => track.id === selectedId) ??
    inScene.find((track) => track.is_default) ??
    inScene[0] ??
    null
  );
}

/** 把毫秒写成 `分:秒`。 */
export function formatDuration(durationMs: number): string {
  const total = Math.max(0, Math.round(durationMs / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
