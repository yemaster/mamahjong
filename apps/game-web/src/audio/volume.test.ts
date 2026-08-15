import { afterEach, describe, expect, it, vi } from "vitest";

class FakeAudio {
  static instances = new Map<string, FakeAudio>();

  preload = "";
  volume = 1;
  loop = false;
  currentTime = 0;
  readyState = 4;
  paused = true;

  constructor(readonly src: string) {
    FakeAudio.instances.set(src, this);
  }

  addEventListener() {}
  removeEventListener() {}
  load() {}
  pause() {
    this.paused = true;
  }
  play() {
    this.paused = false;
    return Promise.resolve();
  }
}

describe("播放中音量设置", () => {
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
    FakeAudio.instances.clear();
    localStorage.clear();
    vi.resetModules();
  });

  it("实时更新背景音乐、角色语音和音效实例", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("Audio", FakeAudio);
    const [{ useAudioSettings }, music, voice, sfx] = await Promise.all([
      import("../stores/audioSettingsStore"),
      import("./music"),
      import("./voice"),
      import("./sfx"),
    ]);

    music.playMusic("/bg.mp3");
    voice.playVoice("/voice.mp3");
    void sfx.preloadSfx("/sfx.mp3", 0);
    sfx.playSfx("/sfx.mp3");
    await Promise.resolve();

    useAudioSettings.getState().setMusicVolume(0.2);
    useAudioSettings.getState().setVoiceVolume(0.4);
    useAudioSettings.getState().setSfxVolume(0.3);

    expect(FakeAudio.instances.get("/bg.mp3")?.volume).toBeCloseTo(0.09);
    expect(FakeAudio.instances.get("/voice.mp3")?.volume).toBeCloseTo(0.36);
    expect(FakeAudio.instances.get("/sfx.mp3")?.volume).toBeCloseTo(0.3);

    /* 对局中立直曲接管背景音乐后，滑块也必须改正在响的立直曲。 */
    music.playRiichiMusic("/riichi.mp3");
    await Promise.resolve();
    useAudioSettings.getState().setMusicVolume(0.6);
    expect(FakeAudio.instances.get("/riichi.mp3")?.volume).toBeCloseTo(0.27);
    expect(FakeAudio.instances.get("/bg.mp3")?.volume).toBe(0);
  });
});
