import { useEffect, useRef, useState } from "react";
import {
  CircleHelp,
  LogOut,
  Maximize2,
  Minimize2,
  Music4,
  ScrollText,
  UserRound,
} from "lucide-react";
import { apiFailure, gameApi } from "../api";
import { resumeCurrentActivity } from "../activity";
import { LobbyFunctionButton } from "../components/LobbyFunctionButton";
import { useSceneReady } from "../components/SceneTransition";
import { navigateTo } from "../routing";
import {
  returnToSplashForLogin,
  useAuthStore,
} from "../stores/authStore";
import { CreateRoomPanel } from "./lobby/CreateRoomPanel";

const lobbyBackground = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;
const fallbackCharacter = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/outfits/yiji.png`;
const fallbackAvatar = `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

type Menu = "main" | "ranked" | "friends" | "join" | "create";

export default function LobbyScene({
  initialMenu = "main",
}: {
  initialMenu?: Menu;
}) {
  const token = useAuthStore((state) => state.token);
  const identity = useAuthStore((state) => state.identity);
  const [menu, setMenu] = useState<Menu>(initialMenu);
  const [roomId, setRoomId] = useState("");
  const [joining, setJoining] = useState(false);
  const [joinError, setJoinError] = useState<string | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const [character, setCharacter] = useState({
    name: "一姬",
    illustration_path: fallbackCharacter,
    avatar_path: fallbackAvatar,
  });
  const [characterReady, setCharacterReady] = useState(false);
  useSceneReady(characterReady);

  useEffect(() => {
    const syncFullscreen = () =>
      setFullscreen(Boolean(document.fullscreenElement));
    syncFullscreen();
    document.addEventListener("fullscreenchange", syncFullscreen);
    return () =>
      document.removeEventListener("fullscreenchange", syncFullscreen);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const loadImage = (src: string) =>
      new Promise<void>((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve();
        image.onerror = () => reject(new Error("角色立绘加载失败"));
        image.src = src;
      });

    gameApi
      .characters()
      .then(async ({ characters }) => {
        const selectedCharacterId = identity?.profile.selected_character?.id;
        const nextCharacter =
          characters.find(
            (candidate) => candidate.id === selectedCharacterId,
          ) ??
          characters.find((candidate) => candidate.is_default) ??
          characters[0];
        if (!nextCharacter) {
          throw new Error("没有可用角色");
        }
        const illustrationPath =
          nextCharacter.outfits.find(
            (outfit) => outfit.id === identity?.profile.selected_outfit_id,
          )?.illustration_path ?? nextCharacter.illustration_path;
        const avatarPath =
          nextCharacter.emotes.find(
            (emote) => emote.path === identity?.profile.avatar_path,
          )?.path ??
          nextCharacter.emotes.find((emote) => emote.name === "微笑")?.path ??
          nextCharacter.emotes[0]?.path ??
          illustrationPath;
        await Promise.all([
          loadImage(illustrationPath),
          loadImage(avatarPath),
        ]);
        if (!cancelled) {
          setCharacter({
            name: nextCharacter.name,
            illustration_path: illustrationPath,
            avatar_path: avatarPath,
          });
          setCharacterReady(true);
        }
      })
      .catch(async () => {
        await Promise.all([
          loadImage(fallbackCharacter).catch(() => {}),
          loadImage(fallbackAvatar).catch(() => {}),
        ]);
        if (!cancelled) setCharacterReady(true);
      });

    return () => {
      cancelled = true;
    };
  }, [
    identity?.profile.avatar_path,
    identity?.profile.selected_character?.id,
    identity?.profile.selected_outfit_id,
  ]);

  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch {
      // 浏览器可能拒绝全屏请求。
    }
  };

  const logoutToSplash = () => {
    returnToSplashForLogin();
  };

  const quickMatch = (variant: string) => {
    if (!token) return;
    gameApi
      .enterMatchmaking(`riichi/${variant}`, token)
      .then((ticket) =>
        navigateTo({ kind: "matchmaking", ticketId: ticket.id }),
      )
      .catch(async (error: unknown) => {
        if (apiFailure(error).code === "lobby.user_busy") {
          await resumeCurrentActivity(token).catch(() => false);
        }
      });
  };

  const joinFriendRoom = async () => {
    const targetRoomId = roomId.trim();
    if (
      !token ||
      !identity ||
      !/^\d{6}$/.test(targetRoomId) ||
      joining
    ) {
      return;
    }

    setJoining(true);
    setJoinError(null);
    try {
      await gameApi.getRoom(targetRoomId, token);
      navigateTo({ kind: "room", roomId: targetRoomId });
    } catch (error: unknown) {
      if (apiFailure(error).code === "lobby.user_busy") {
        const resumed = await resumeCurrentActivity(token).catch(
          () => false,
        );
        if (resumed) return;
      }
      setJoinError("加入失败，请检查房间编号");
    } finally {
      setJoining(false);
    }
  };

  return (
    <section className="game-lobby" aria-label="雀庄大厅">
      <div
        className="game-lobby__background"
        style={{ backgroundImage: `url("${lobbyBackground}")` }}
        aria-hidden="true"
      />
      <div className="game-lobby__veil" aria-hidden="true" />

      <div className="game-lobby__character">
        <img src={character.illustration_path} alt={character.name} />
      </div>

      <button
        type="button"
        className="game-lobby__user-card"
        onClick={() =>
          navigateTo({ kind: "profile", userId: identity?.id })
        }
        aria-label="查看用户详情"
      >
        <div className="game-lobby__user-avatar">
          <img src={character.avatar_path} alt={character.name} />
        </div>
        <div className="game-lobby__user-name">
          {identity?.profile.nickname || identity?.login_name}
        </div>
      </button>

      <div className="game-lobby__utility" aria-label="功能按钮">
        <button
          type="button"
          onClick={() => navigateTo({ kind: "yaku-reference" })}
          aria-label="帮助"
          title="帮助"
        >
          <CircleHelp aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={() => void toggleFullscreen()}
          aria-label={fullscreen ? "退出全屏" : "进入全屏"}
          title={fullscreen ? "退出全屏" : "进入全屏"}
        >
          {fullscreen ? (
            <Minimize2 aria-hidden="true" />
          ) : (
            <Maximize2 aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          onClick={logoutToSplash}
          aria-label="退出登录"
          title="退出登录"
        >
          <LogOut aria-hidden="true" />
        </button>
      </div>

      {/* 靠右排，从右往左依次是牌谱、音乐、角色。 */}
      <div className="game-lobby__functions" aria-label="功能">
        <LobbyFunctionButton
          icon={UserRound}
          label="角色"
          onClick={() => navigateTo({ kind: "profile", tab: "character" })}
        />
        <LobbyFunctionButton
          icon={Music4}
          label="音乐"
          onClick={() => navigateTo({ kind: "profile", tab: "music" })}
        />
        <LobbyFunctionButton
          icon={ScrollText}
          label="牌谱"
          onClick={() => navigateTo({ kind: "records" })}
        />
      </div>

      <LobbyMenu
        key={menu}
        menu={menu}
        roomId={roomId}
        joining={joining}
        token={token}
        onMenuChange={(nextMenu) => {
          setJoinError(null);
          setMenu(nextMenu);
        }}
        onRoomIdChange={setRoomId}
        onJoin={joinFriendRoom}
        onQuickMatch={quickMatch}
      />
      {joinError && (
        <JoinErrorDialog
          message={joinError}
          onClose={() => setJoinError(null)}
        />
      )}
    </section>
  );
}

function LobbyMenu({
  menu,
  roomId,
  joining,
  token,
  onMenuChange,
  onRoomIdChange,
  onJoin,
  onQuickMatch,
}: {
  menu: Menu;
  roomId: string;
  joining: boolean;
  token: string | null;
  onMenuChange: (menu: Menu) => void;
  onRoomIdChange: (value: string) => void;
  onJoin: () => void;
  onQuickMatch: (variant: string) => void;
}) {
  if (menu === "main") {
    return (
      <nav className="game-lobby__menu" aria-label="游戏菜单">
        <LobbyMenuButton
          label="段位匹配"
          mark="段"
          onClick={() => onMenuChange("ranked")}
        />
        <LobbyMenuButton
          label="好友对战"
          mark="友"
          onClick={() => onMenuChange("friends")}
        />
      </nav>
    );
  }

  if (menu === "ranked") {
    return (
      <nav className="game-lobby__menu" aria-label="段位匹配">
        <LobbyMenuButton
          label="四人半庄"
          mark="四"
          onClick={() => onQuickMatch("yonma")}
        />
        <LobbyMenuButton
          label="三人半庄"
          mark="三"
          onClick={() => onQuickMatch("sanma")}
        />
        <LobbyBackButton onClick={() => onMenuChange("main")} />
      </nav>
    );
  }

  if (menu === "friends") {
    return (
      <nav className="game-lobby__menu" aria-label="好友对战">
        <LobbyMenuButton
          label="创建房间"
          mark="创"
          onClick={() => onMenuChange("create")}
        />
        <LobbyMenuButton
          label="加入房间"
          mark="入"
          onClick={() => onMenuChange("join")}
        />
        <LobbyBackButton onClick={() => onMenuChange("main")} />
      </nav>
    );
  }

  if (menu === "create") {
    return (
      <CreateRoomPanel
        token={token}
        onBack={() => onMenuChange("friends")}
      />
    );
  }

  return (
    <form
      className="game-lobby__menu game-lobby__join"
      aria-label="加入房间"
      onSubmit={(event) => {
        event.preventDefault();
        onJoin();
      }}
    >
      <header className="game-lobby__join-header">
        <span aria-hidden="true">入</span>
        <h2>加入房间</h2>
      </header>
      <div className="game-lobby__join-body">
        <label htmlFor="friend-room-id">房间号</label>
        <input
          id="friend-room-id"
          value={roomId}
          onChange={(event) =>
            onRoomIdChange(event.target.value.replace(/\D/g, "").slice(0, 6))
          }
          autoComplete="off"
          inputMode="numeric"
          maxLength={6}
          placeholder="六位房间号"
          autoFocus
        />
      </div>
      <footer className="game-lobby__join-actions">
        <LobbyBackButton onClick={() => onMenuChange("friends")} />
        <button
          type="submit"
          className="game-lobby__join-submit"
          disabled={!/^\d{6}$/.test(roomId.trim()) || joining}
        >
          {joining ? "加入中" : "加入房间"}
        </button>
      </footer>
    </form>
  );
}

function JoinErrorDialog({
  message,
  onClose,
}: {
  message: string;
  onClose: () => void;
}) {
  const confirmButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    confirmButton.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <div
      className="game-lobby__notice-overlay"
      role="presentation"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) onClose();
      }}
    >
      <section
        className="game-lobby__notice"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="join-error-title"
        aria-describedby="join-error-message"
      >
        <h2 id="join-error-title">提示</h2>
        <p id="join-error-message">{message}</p>
        <button ref={confirmButton} type="button" onClick={onClose}>
          确定
        </button>
      </section>
    </div>
  );
}

function LobbyMenuButton({
  label,
  mark,
  onClick,
}: {
  label: string;
  mark: string;
  onClick: () => void;
}) {
  return (
    <button type="button" className="game-lobby__menu-button" onClick={onClick}>
      <span>
        <b aria-hidden="true">{mark}</b>
        <i>{label}</i>
      </span>
    </button>
  );
}

function LobbyBackButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="game-lobby__menu-back" onClick={onClick}>
      返回
    </button>
  );
}
