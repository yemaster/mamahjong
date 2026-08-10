import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { apiFailure, gameApi } from "../api";
import { useSceneReady } from "../components/SceneTransition";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import {
  formatDuration,
  previewMusic,
  resolveTrack,
  stopPreview,
} from "../audio/music";
import {
  actionVoices,
  previewVoice,
  stopVoicePreview,
} from "../audio/voice";
import type {
  CharacterVoice,
  LobbyCharacter,
  MusicScene,
  MusicTrackView,
  PlayerStatistics,
  UserView,
} from "../types";

const profileBackground = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;
const fallbackAvatar = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

type ProfileTab = "info" | "character" | "interface" | "music";

export default function ProfileScene({
  userId,
  initialTab = "info",
  returnRoomId,
}: {
  userId?: string;
  initialTab?: ProfileTab;
  returnRoomId?: string;
}) {
  const { identity, token, setIdentity } = useAuthStore();
  const [activeTab, setActiveTab] = useState<ProfileTab>(initialTab);
  const targetUserId = userId ?? identity?.id ?? "";
  const isOwn = targetUserId === identity?.id;
  const profile = useQuery({
    queryKey: ["profile", targetUserId],
    queryFn: () => gameApi.profile(token!, targetUserId),
    enabled: Boolean(token && targetUserId),
  });
  const characters = useQuery({
    queryKey: ["characters"],
    queryFn: gameApi.characters,
    enabled: isOwn,
  });
  useSceneReady(!profile.isLoading && (!isOwn || !characters.isLoading));

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab]);

  const goBack = () =>
    navigateTo(
      returnRoomId
        ? { kind: "room", roomId: returnRoomId }
        : { kind: "lobby" },
    );

  if (profile.isLoading || (isOwn && characters.isLoading)) {
    return <div className="profile-screen__loading">加载中…</div>;
  }
  if (profile.error || !profile.data) {
    return (
      <div className="profile-screen__error">
        <p>{profile.error ? apiFailure(profile.error).message : "用户不存在"}</p>
        <button type="button" onClick={goBack}>
          {returnRoomId ? "返回房间" : "返回大厅"}
        </button>
      </div>
    );
  }

  const user = profile.data.user;
  const availableCharacters = characters.data?.characters ?? [];
  const selectedCharacter =
    availableCharacters.find(
      (character) => character.id === user.profile.selected_character?.id,
    ) ??
    availableCharacters.find((character) => character.is_default) ??
    availableCharacters[0];
  const avatarPath =
    selectedCharacter?.emotes.find(
      (emote) => emote.path === user.profile.avatar_path,
    )?.path ??
    selectedCharacter?.emotes.find((emote) => emote.name === "微笑")?.path ??
    selectedCharacter?.emotes[0]?.path ??
    fallbackAvatar;

  return (
    <section className="profile-screen" aria-label="用户详情">
      <div
        className="profile-screen__background"
        style={{ backgroundImage: `url("${profileBackground}")` }}
        aria-hidden="true"
      />
      <div className="profile-screen__veil" aria-hidden="true" />

      <div className="profile-screen__content">
        <header className="profile-screen__header">
          <button type="button" onClick={goBack}>
            {returnRoomId ? "返回房间" : "返回大厅"}
          </button>
          <h1>用户详情</h1>
        </header>

        <div className="profile-screen__layout">
          <IdentityPanel
            user={user}
            avatarPath={avatarPath}
            character={selectedCharacter}
            isOwn={isOwn}
            onUpdated={() => void profile.refetch()}
          />

          <div
            className={`profile-screen__workspace${isOwn ? " has-tabs" : ""}`}
          >
            {isOwn && (
              <div
                className="profile-screen__tabs"
                role="tablist"
                aria-label="用户详情分类"
              >
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeTab === "info"}
                  className={activeTab === "info" ? "is-active" : ""}
                  onClick={() => setActiveTab("info")}
                >
                  用户信息
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeTab === "character"}
                  className={activeTab === "character" ? "is-active" : ""}
                  onClick={() => setActiveTab("character")}
                >
                  角色设置
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeTab === "interface"}
                  className={activeTab === "interface" ? "is-active" : ""}
                  onClick={() => setActiveTab("interface")}
                >
                  界面设置
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeTab === "music"}
                  className={activeTab === "music" ? "is-active" : ""}
                  onClick={() => setActiveTab("music")}
                >
                  音乐设置
                </button>
              </div>
            )}

            <div className="profile-screen__main">
              {activeTab === "character" && isOwn ? (
                <PresentationEditor
                  characters={availableCharacters}
                  initialCharacterId={
                    user.profile.selected_character?.id ??
                    selectedCharacter?.id ??
                    ""
                  }
                  initialOutfitId={
                    user.profile.selected_outfit_id ??
                    selectedCharacter?.outfits[0]?.id ??
                    ""
                  }
                  initialAvatarPath={avatarPath}
                  onSaved={(updatedUser) => {
                    setIdentity(updatedUser);
                    void profile.refetch();
                  }}
                />
              ) : activeTab === "interface" && isOwn ? (
                <InterfaceSettings />
              ) : activeTab === "music" && isOwn ? (
                <MusicSettings />
              ) : (
                <>
                  <StatisticsPanel statistics={profile.data.statistics} />
                  <RecentMatches statistics={profile.data.statistics} />
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function InterfaceSettings() {
  const { identity, token, setIdentity } = useAuthStore();
  const tablecloths = useQuery({
    queryKey: ["tablecloths"],
    queryFn: gameApi.tablecloths,
  });
  const defaultTablecloth =
    tablecloths.data?.tablecloths.find((tablecloth) => tablecloth.is_default) ??
    tablecloths.data?.tablecloths[0];
  const [selectedId, setSelectedId] = useState(
    identity?.profile.selected_tablecloth_id ?? "",
  );
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedId && defaultTablecloth) {
      setSelectedId(
        identity?.profile.selected_tablecloth_id ?? defaultTablecloth.id,
      );
    }
  }, [
    defaultTablecloth,
    identity?.profile.selected_tablecloth_id,
    selectedId,
  ]);

  const saveTablecloth = async () => {
    if (!token || !selectedId || saving) return;
    setSaving(true);
    setMessage(null);
    try {
      const updated = await gameApi.updateTablecloth(token, selectedId);
      setIdentity(updated);
      setMessage("桌布已保存");
    } catch {
      setMessage("桌布保存失败");
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="profile-section profile-interface-settings">
      <h2>界面设置</h2>
      <button
        type="button"
        className="profile-interface-settings__entry"
        onClick={() => navigateTo({ kind: "table-settings" })}
      >
        <span>
          <strong>牌桌设置</strong>
          <small>调整对局镜头</small>
        </span>
        <b>进入</b>
      </button>
      <div className="profile-tablecloth-settings">
        <h3>桌布</h3>
        <div className="profile-tablecloth-settings__grid">
          {tablecloths.data?.tablecloths.map((tablecloth) => (
            <button
              type="button"
              key={tablecloth.id}
              className={selectedId === tablecloth.id ? "is-selected" : ""}
              onClick={() => {
                setSelectedId(tablecloth.id);
                setMessage(null);
              }}
            >
              <img src={tablecloth.texture_path} alt="" />
              <span>{tablecloth.name}</span>
            </button>
          ))}
        </div>
        <div className="profile-tablecloth-settings__actions">
          {message && <span>{message}</span>}
          <button
            type="button"
            disabled={
              saving ||
              !selectedId ||
              selectedId === identity?.profile.selected_tablecloth_id
            }
            onClick={() => void saveTablecloth()}
          >
            {saving ? "保存中…" : "保存桌布"}
          </button>
        </div>
      </div>
    </section>
  );
}

function MusicSettings() {
  const { identity, token, setIdentity } = useAuthStore();
  const music = useQuery({
    queryKey: ["music-tracks"],
    queryFn: () => gameApi.musicTracks(),
    staleTime: 5 * 60_000,
  });
  const tracks = useMemo(
    () => music.data?.music_tracks ?? [],
    [music.data],
  );
  const savedLobbyId = identity?.profile.selected_lobby_music_id ?? null;
  const savedMatchId = identity?.profile.selected_match_music_id ?? null;
  const savedRiichiId = identity?.profile.selected_riichi_music_id ?? null;
  const [lobbyId, setLobbyId] = useState(savedLobbyId ?? "");
  const [matchId, setMatchId] = useState(savedMatchId ?? "");
  const [riichiId, setRiichiId] = useState(savedRiichiId ?? "");
  const [failed, setFailed] = useState(false);
  /* 正在试听/加载的那一首。 */
  const [playingId, setPlayingId] = useState<string | null>(null);
  const [loadingId, setLoadingId] = useState<string | null>(null);

  /* 没选过就落在该场景的默认曲上。 */
  useEffect(() => {
    if (tracks.length === 0) return;
    setLobbyId(
      (current) =>
        current || resolveTrack(tracks, "lobby", savedLobbyId)?.id || "",
    );
    setMatchId(
      (current) =>
        current || resolveTrack(tracks, "match", savedMatchId)?.id || "",
    );
  }, [savedLobbyId, savedMatchId, tracks]);

  /* 离开这一页就把试听掐掉。 */
  useEffect(() => () => stopPreview(), []);

  /* 选中即播，选"无"（id 为空）就不播。 */
  const preview = async (id: string) => {
    if (!id) return;
    const track = tracks.find((t) => t.id === id);
    if (!track) return;
    setLoadingId(id);
    setPlayingId(null);
    await previewMusic(track.audio_path, () => {
      setPlayingId((current) => (current === id ? null : current));
    });
    setLoadingId(null);
    setPlayingId(id);
  };

  /* 点哪首就定哪首，同时自动试听；发不出去就退回原来那首。 */
  const choose = async (scene: MusicScene, id: string) => {
    const apply =
      scene === "lobby"
        ? setLobbyId
        : scene === "match"
          ? setMatchId
          : setRiichiId;
    const previous =
      scene === "lobby" ? lobbyId : scene === "match" ? matchId : riichiId;
    if (!token || id === previous) return;
    apply(id);
    setFailed(false);
    void preview(id);
    const body: { lobby_music_id?: string; match_music_id?: string; riichi_music_id?: string } = {};
    if (scene === "lobby") body.lobby_music_id = id;
    else if (scene === "match") body.match_music_id = id;
    else body.riichi_music_id = id;
    try {
      const updated = await gameApi.updateMusic(token, body);
      setIdentity(updated);
    } catch {
      apply(previous);
      setFailed(true);
    }
  };

  return (
    <section className="profile-section profile-music-settings">
      <h2>音乐设置</h2>
      <MusicPicker
        title="大厅音乐"
        scene="lobby"
        tracks={tracks}
        selectedId={lobbyId}
        playingId={playingId}
        loadingId={loadingId}
        onSelect={(id) => void choose("lobby", id)}
      />
      <MusicPicker
        title="对局音乐"
        scene="match"
        tracks={tracks}
        selectedId={matchId}
        playingId={playingId}
        loadingId={loadingId}
        onSelect={(id) => void choose("match", id)}
      />
      <RiichiMusicPicker
        title="立直音乐"
        tracks={tracks}
        selectedId={riichiId}
        playingId={playingId}
        loadingId={loadingId}
        onSelect={(id) => void choose("riichi", id)}
      />
      {failed && <p className="profile-music-settings__failed">曲目未能保存</p>}
    </section>
  );
}

function RiichiMusicPicker({
  title,
  tracks,
  selectedId,
  playingId,
  loadingId,
  onSelect,
}: {
  title: string;
  tracks: MusicTrackView[];
  selectedId: string;
  playingId: string | null;
  loadingId: string | null;
  onSelect: (id: string) => void;
}) {
  const list = tracks.filter((track) => track.scene === "riichi");

  return (
    <div className="music-picker">
      <h3>{title}</h3>
      <ul className="music-picker__list">
        {/* 无立直音乐 */}
        <li className={selectedId === "" ? "is-selected" : ""}>
          <button
            type="button"
            className="music-picker__pick"
            onClick={() => onSelect("")}
          >
            <span className="music-picker__name">无</span>
            {selectedId === "" && (
              <span className="music-picker__mark">选用</span>
            )}
          </button>
        </li>
        {list.length === 0 ? (
          <p className="music-picker__empty">暂无曲目</p>
        ) : (
          list.map((track) => {
            const isLoading = loadingId === track.id;
            const isPlaying = playingId === track.id;
            return (
              <li
                key={track.id}
                className={selectedId === track.id ? "is-selected" : ""}
              >
                <button
                  type="button"
                  className="music-picker__pick"
                  onClick={() => onSelect(track.id)}
                >
                  <span className="music-picker__name">{track.name}</span>
                  <span className="music-picker__time">
                    {formatDuration(track.duration_ms)}
                  </span>
                  {selectedId === track.id && (
                    <span className="music-picker__mark">选用</span>
                  )}
                  {isLoading && (
                    <i className="music-picker__spinner" aria-label="载入中" />
                  )}
                  {isPlaying && (
                    <span className="music-picker__playing" aria-label="试听中">♪</span>
                  )}
                </button>
              </li>
            );
          })
        )}
      </ul>
    </div>
  );
}

function MusicPicker({
  title,
  scene,
  tracks,
  selectedId,
  playingId,
  loadingId,
  onSelect,
}: {
  title: string;
  scene: MusicScene;
  tracks: MusicTrackView[];
  selectedId: string;
  playingId: string | null;
  loadingId: string | null;
  onSelect: (id: string) => void;
}) {
  const list = tracks.filter((track) => track.scene === scene);

  return (
    <div className="music-picker">
      <h3>{title}</h3>
      {list.length === 0 ? (
        <p className="music-picker__empty">暂无曲目</p>
      ) : (
        <ul className="music-picker__list">
          {list.map((track) => {
            const isLoading = loadingId === track.id;
            const isPlaying = playingId === track.id;
            return (
              <li
                key={track.id}
                className={selectedId === track.id ? "is-selected" : ""}
              >
                <button
                  type="button"
                  className="music-picker__pick"
                  onClick={() => onSelect(track.id)}
                >
                  <span className="music-picker__name">{track.name}</span>
                  <span className="music-picker__time">
                    {formatDuration(track.duration_ms)}
                  </span>
                  {selectedId === track.id && (
                    <span className="music-picker__mark">选用</span>
                  )}
                  {isLoading && (
                    <i className="music-picker__spinner" aria-label="载入中" />
                  )}
                  {isPlaying && (
                    <span className="music-picker__playing" aria-label="试听中">♪</span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

function IdentityPanel({
  user,
  avatarPath,
  character,
  isOwn,
  onUpdated,
}: {
  user: UserView;
  avatarPath: string;
  character?: LobbyCharacter;
  isOwn: boolean;
  onUpdated: () => void;
}) {
  const { identity, token, setIdentity } = useAuthStore();
  const displayedUser = isOwn ? identity ?? user : user;
  const [editing, setEditing] = useState(false);
  const [nickname, setNickname] = useState(displayedUser?.profile.nickname ?? "");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const saveNickname = async () => {
    if (!token || !nickname.trim() || saving) return;
    setSaving(true);
    setMessage(null);
    try {
      const updated = await gameApi.updateProfile(token, nickname.trim());
      setIdentity(updated);
      setEditing(false);
      setMessage("昵称已保存");
      onUpdated();
    } catch {
      setMessage("昵称保存失败");
    } finally {
      setSaving(false);
    }
  };

  return (
    <aside className="profile-identity">
      <div className="profile-identity__avatar">
        <img src={avatarPath} alt="用户头像" />
      </div>
      <h2>{displayedUser?.profile.nickname ?? "雀士"}</h2>
      <span className="profile-identity__character">
        {character?.name ?? displayedUser?.profile.selected_character?.name ?? "未选择角色"}
      </span>

      <dl className="profile-identity__details">
        <div>
          <dt>账号</dt>
          <dd>{displayedUser?.login_name ?? "—"}</dd>
        </div>
        <div>
          <dt>身份</dt>
          <dd>{displayedUser?.role === "administrator" ? "管理员" : "玩家"}</dd>
        </div>
        <div>
          <dt>段位</dt>
          <dd>
            {displayedUser?.profile.ranks[0]?.rank ?? "尚未定级"}
          </dd>
        </div>
      </dl>

      {isOwn && (
        <div className="profile-identity__edit">
          {editing ? (
            <>
              <input
                value={nickname}
                maxLength={24}
                aria-label="昵称"
                onChange={(event) => setNickname(event.target.value)}
                onKeyDown={(event) =>
                  event.key === "Enter" && void saveNickname()
                }
              />
              <div>
                <button
                  type="button"
                  onClick={() => void saveNickname()}
                  disabled={saving}
                >
                  保存
                </button>
                <button type="button" onClick={() => setEditing(false)}>
                  取消
                </button>
              </div>
            </>
          ) : (
            <button type="button" onClick={() => setEditing(true)}>
              修改昵称
            </button>
          )}
          {message && <span>{message}</span>}
        </div>
      )}
    </aside>
  );
}

function StatisticsPanel({
  statistics,
}: {
  statistics: PlayerStatistics;
}) {
  const items = [
    ["对局数", String(statistics.matches_played)],
    [
      "平均顺位",
      statistics.matches_played ? statistics.average_rank.toFixed(2) : "—",
    ],
    [
      "一位率",
      percentage(statistics.first_places, statistics.matches_played),
    ],
    ["和牌率", percentage(statistics.wins, statistics.hands_played)],
    ["放铳率", percentage(statistics.deal_ins, statistics.hands_played)],
    ["立直率", percentage(statistics.riichi_count, statistics.hands_played)],
    ["荣和", String(statistics.ron_wins)],
    ["自摸", String(statistics.tsumo_wins)],
    [
      "最高得点",
      statistics.highest_hand_gain
        ? `+${statistics.highest_hand_gain.toLocaleString("zh-CN")}`
        : "—",
    ],
  ];
  return (
    <section className="profile-section">
      <h2>战绩统计</h2>
      <div className="profile-stat-grid">
        {items.map(([label, value]) => (
          <div key={label} className="profile-stat">
            <span>{label}</span>
            <strong>{value}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

function RecentMatches({ statistics }: { statistics: PlayerStatistics }) {
  return (
    <section className="profile-section">
      <h2>最近战绩</h2>
      {statistics.recent_matches.length ? (
        <div className="profile-records">
          {statistics.recent_matches.map((record) => (
            <div key={record.match_id} className="profile-record">
              <strong className={`is-rank-${record.rank}`}>
                第{record.rank}位
              </strong>
              <span>{record.final_points.toLocaleString("zh-CN")}点</span>
              <span>{record.hands}局</span>
              <span>{record.wins}次和牌</span>
            </div>
          ))}
        </div>
      ) : (
        <div className="profile-section__empty">暂无完成的对局</div>
      )}
    </section>
  );
}

/** 一份完整的形象。三项一起发，服务端那个接口本来也只收整份。 */
interface Presentation {
  characterId: string;
  outfitId: string;
  avatarPath: string;
}

/** 换角色时该落在哪个头像上：优先「微笑」，没有就用第一个。 */
function defaultAvatarPath(character: LobbyCharacter): string {
  return (
    character.emotes.find((emote) => emote.name === "微笑")?.path ??
    character.emotes[0]?.path ??
    ""
  );
}

function PresentationEditor({
  characters,
  initialCharacterId,
  initialOutfitId,
  initialAvatarPath,
  onSaved,
}: {
  characters: LobbyCharacter[];
  initialCharacterId: string;
  initialOutfitId: string;
  initialAvatarPath: string;
  onSaved: (user: Awaited<ReturnType<typeof gameApi.updatePresentation>>) => void;
}) {
  const token = useAuthStore((state) => state.token);
  const [selection, setSelection] = useState<Presentation>({
    characterId: initialCharacterId,
    outfitId: initialOutfitId,
    avatarPath: initialAvatarPath,
  });
  const [failed, setFailed] = useState(false);
  /* 连点几下时只认最后一次的结果，早到的回执不许把新选择盖回去。 */
  const requestSequence = useRef(0);
  const selectedCharacter = useMemo(
    () => characters.find((character) => character.id === selection.characterId),
    [characters, selection.characterId],
  );

  /* 存量数据可能对不上号（换过角色、皮肤被删了），进来先把本地状态摆正。
     只改显示，不往服务端写——光是打开这一页不该产生一次保存。 */
  useEffect(() => {
    if (!selectedCharacter) return;
    setSelection((current) => {
      const outfitValid = selectedCharacter.outfits.some(
        (outfit) => outfit.id === current.outfitId,
      );
      const avatarValid = selectedCharacter.emotes.some(
        (emote) => emote.path === current.avatarPath,
      );
      if (outfitValid && avatarValid) return current;
      return {
        characterId: current.characterId,
        outfitId: outfitValid
          ? current.outfitId
          : (selectedCharacter.outfits[0]?.id ?? ""),
        avatarPath: avatarValid
          ? current.avatarPath
          : defaultAvatarPath(selectedCharacter),
      };
    });
  }, [selectedCharacter]);

  /* 选了就存，不再等「保存」按钮。先把界面切过去，存不上再退回来。 */
  const apply = async (next: Presentation) => {
    if (!token || !next.characterId || !next.outfitId || !next.avatarPath) {
      return;
    }
    const previous = selection;
    requestSequence.current += 1;
    const sequence = requestSequence.current;
    setSelection(next);
    setFailed(false);
    try {
      const updated = await gameApi.updatePresentation(
        token,
        next.characterId,
        next.outfitId,
        next.avatarPath,
      );
      if (requestSequence.current !== sequence) return;
      onSaved(updated);
    } catch {
      if (requestSequence.current !== sequence) return;
      setSelection(previous);
      setFailed(true);
    }
  };

  /* 换角色连皮肤和头像一起换掉：上一个角色的那两样在新角色身上不存在。 */
  const pickCharacter = (character: LobbyCharacter) => {
    if (character.id === selection.characterId) return;
    void apply({
      characterId: character.id,
      outfitId: character.outfits[0]?.id ?? "",
      avatarPath: defaultAvatarPath(character),
    });
  };

  return (
    <section className="profile-section profile-presentation">
      <h2>形象设置</h2>
      <h3>选择角色</h3>
      <div className="profile-character-options">
        {characters.map((character) => (
          <button
            type="button"
            key={character.id}
            className={
              character.id === selection.characterId ? "is-selected" : ""
            }
            onClick={() => pickCharacter(character)}
          >
            <img src={character.illustration_path} alt="" />
            <span>{character.name}</span>
          </button>
        ))}
      </div>

      <h3>选择皮肤</h3>
      <div className="profile-outfit-options">
        {selectedCharacter?.outfits.map((outfit) => (
          <button
            type="button"
            key={outfit.id}
            className={outfit.id === selection.outfitId ? "is-selected" : ""}
            onClick={() => void apply({ ...selection, outfitId: outfit.id })}
          >
            <img src={outfit.illustration_path} alt="" />
            <span>{outfit.name}</span>
          </button>
        ))}
      </div>

      <h3>选择头像</h3>
      <div className="profile-avatar-options">
        {selectedCharacter?.emotes.map((avatar) => (
          <button
            type="button"
            key={avatar.path}
            className={avatar.path === selection.avatarPath ? "is-selected" : ""}
            onClick={() => void apply({ ...selection, avatarPath: avatar.path })}
            title={avatar.name}
          >
            <img src={avatar.path} alt={avatar.name} />
          </button>
        ))}
      </div>

      <VoicePicker character={selectedCharacter} />

      {failed && <p className="profile-presentation__failed">形象未能保存</p>}
    </section>
  );
}

/** 当前角色在牌桌上会喊的那几条，点一下听一遍。 */
function VoicePicker({ character }: { character?: LobbyCharacter }) {
  const [playingPath, setPlayingPath] = useState<string | null>(null);
  const [loadingPath, setLoadingPath] = useState<string | null>(null);
  const requested = useRef<string | null>(null);
  const voices = useMemo(
    () => (character ? actionVoices(character) : []),
    [character],
  );

  /* 换角色或者离开这一页，正在响的那条就掐掉。 */
  useEffect(() => {
    stopVoicePreview();
    setPlayingPath(null);
    setLoadingPath(null);
    requested.current = null;
  }, [character?.id]);
  useEffect(() => () => stopVoicePreview(), []);

  const toggle = async (voice: CharacterVoice) => {
    if (playingPath === voice.path) {
      stopVoicePreview();
      setPlayingPath(null);
      return;
    }
    requested.current = voice.path;
    setLoadingPath(voice.path);
    setPlayingPath(null);
    await previewVoice(voice.path, () => {
      setPlayingPath((current) => (current === voice.path ? null : current));
    });
    if (requested.current !== voice.path) return;
    setLoadingPath(null);
    setPlayingPath(voice.path);
  };

  return (
    <div className="voice-picker">
      <h3>语音试听</h3>
      {voices.length === 0 ? (
        <p className="voice-picker__empty">该角色暂无操作语音</p>
      ) : (
        <ul className="voice-picker__list">
          {voices.map((voice) => (
            <li
              key={voice.path}
              className={`voice-picker__item${loadingPath === voice.path ? " is-loading" : ""}${playingPath === voice.path ? " is-playing" : ""}`}
              onClick={() => void toggle(voice)}
              title={playingPath === voice.path ? "停止试听" : "试听"}
            >
              <span className="voice-picker__name">{voice.name}</span>
              {loadingPath === voice.path && (
                <i className="voice-picker__spinner" aria-label="载入中" />
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function percentage(value: number, total: number): string {
  if (!total) return "—";
  return `${((value / total) * 100).toFixed(1)}%`;
}
