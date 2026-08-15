import type { CharacterVoice, LobbyCharacter, VoiceKind } from "../types";
import { useAudioSettings } from "../stores/audioSettingsStore";

/**
 * 角色语音的基础音量。要盖过背景音乐（0.45），牌桌上喊出来才听得见。
 */
const BASE_VOICE_VOLUME = 0.9;
/** 试听的基础音量；仍然受玩家的语音音量设置控制。 */
const PREVIEW_VOLUME = 0.9;
function voiceVolume(): number {
  return BASE_VOICE_VOLUME * useAudioSettings.getState().voiceVolume;
}

function previewVolume(): number {
  return PREVIEW_VOLUME * useAudioSettings.getState().voiceVolume;
}

/** 素材没进 git，缺文件时不能把玩家卡在开局加载里，到点就当load完了。 */
const PRELOAD_TIMEOUT_MS = 20_000;

const cache = new Map<string, HTMLAudioElement>();

/** 正在试听的那一条。对局里的喊话不走这里，各喊各的不互相打断。 */
let preview: HTMLAudioElement | null = null;

useAudioSettings.subscribe((current, previous) => {
  if (current.voiceVolume === previous.voiceVolume) return;
  for (const audio of cache.values()) {
    audio.volume = audio === preview ? previewVolume() : voiceVolume();
  }
});

const audioAvailable = () => typeof Audio !== "undefined";

function element(src: string): HTMLAudioElement {
  const cached = cache.get(src);
  if (cached) {
    return cached;
  }
  const audio = new Audio(src);
  audio.preload = "auto";
  audio.volume = voiceVolume();
  cache.set(src, audio);
  return audio;
}

/**
 * 把一条语音load到能连续播放为止。
 *
 * 和音乐那边一样永远 resolve：素材缺失、解码失败、网络太慢都算「load完」，
 * 不然四家里只要有一个人少一个文件，整局就开不出来。
 */
export function preloadVoice(
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

/**
 * 一次性load一批语音，全部落地（或到点）才 resolve。
 *
 * 开局前四家的操作语音都从这里过一遍：真到有人喊的时候文件已经在内存里，
 * 不会出现横幅都弹完了声音才姗姗来迟。
 */
export function preloadVoices(
  sources: readonly string[],
  timeoutMs: number = PRELOAD_TIMEOUT_MS,
): Promise<void> {
  const unique = [...new Set(sources.filter(Boolean))];
  return Promise.all(
    unique.map((src) => preloadVoice(src, timeoutMs)),
  ).then(() => undefined);
}

/**
 * 喊一声。播不出来就当没喊，绝不往上抛。
 *
 * 同一条正在响的时候再喊一次会从头开始：连着两次碰就该听见两次，而不是第二
 * 次被吞掉。
 */
export function playVoice(src: string | null | undefined): void {
  if (!src || !audioAvailable()) {
    return;
  }
  const audio = element(src);
  audio.loop = false;
  audio.volume = voiceVolume();
  try {
    audio.currentTime = 0;
  } catch {
    // 还没load出时长的时候设 currentTime 会抛，忽略，play 之后自然从头放。
  }
  void audio.play().catch(() => undefined);
}

/**
 * 试听一条语音，load完并开始播放后 resolve。
 *
 * 和音乐试听不同，这里不去压背景音乐：语音就一两秒，为它把大厅曲淡出再淡回
 * 来只会让人觉得卡了一下，而且对局里本来就是盖着音乐喊的。
 */
export async function previewVoice(
  src: string,
  onEnded?: () => void,
): Promise<void> {
  if (!audioAvailable()) {
    return;
  }
  stopVoicePreview();
  await preloadVoice(src);
  const audio = element(src);
  audio.loop = false;
  audio.volume = previewVolume();
  try {
    audio.currentTime = 0;
  } catch {
    // 同上，忽略。
  }
  preview = audio;
  const ended = () => {
    audio.removeEventListener("ended", ended);
    if (preview === audio) {
      preview = null;
    }
    onEnded?.();
  };
  audio.addEventListener("ended", ended);
  try {
    await audio.play();
  } catch {
    // 文件缺了或者浏览器还没给手势，当场结束，按钮不要一直转。
    ended();
  }
}

/** 停掉试听。 */
export function stopVoicePreview(): void {
  const audio = preview;
  preview = null;
  if (audio) {
    audio.pause();
    try {
      audio.currentTime = 0;
    } catch {
      // 忽略。
    }
  }
}

/**
 * 从一个角色身上找出某个动作该喊的那一条。
 *
 * 先认 `kind`；老数据没有 `kind`，退回按名字认，免得升级前配好的角色突然哑
 * 掉。两样都对不上就返回 null，调用方当没有这条语音处理。
 */
export function resolveVoice(
  character: LobbyCharacter | undefined,
  kind: VoiceKind,
): string | null {
  if (!character) {
    return null;
  }
  const byKind = character.voices.find((voice) => voice.kind === kind);
  if (byKind) {
    return byKind.path;
  }
  const name = LEGACY_VOICE_NAMES[kind];
  return character.voices.find((voice) => voice.name === name)?.path ?? null;
}

/** 加 `kind` 之前，库里就是靠这些名字区分语音的。 */
const LEGACY_VOICE_NAMES: Record<VoiceKind, string> = {
  riichi: "立直",
  double_riichi: "两立直",
  chi: "吃",
  pon: "碰",
  kan: "杠",
  nuki: "拔北",
  ron: "荣和",
  tsumo: "自摸",
};

/** 一个角色所有带 `kind` 的语音，也就是牌桌上真会喊出来的那些。 */
export function actionVoices(character: LobbyCharacter): CharacterVoice[] {
  return character.voices.filter((voice) => Boolean(voice.kind));
}
