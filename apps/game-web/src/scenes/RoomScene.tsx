import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { ApiError, apiFailure, gameApi } from "../api";
import { resumeCurrentActivity } from "../activity";
import { useSceneReady } from "../components/SceneTransition";
import { navigateTo } from "../routing";
import { roomRuleTitle } from "../ruleTitle";
import { useAuthStore } from "../stores/authStore";
import type { RoomMemberView, RoomView } from "../types";
import { matchToEnter } from "./room/roomEntry";

const roomBackground = `${import.meta.env.BASE_URL}assets/ui/sakura-campus-empty.png`;
const seatNames = ["东", "南", "西", "北"];

interface RoomSceneProps {
  roomId: string;
}

export default function RoomScene({ roomId }: RoomSceneProps) {
  const token = useAuthStore((state) => state.token);
  const identity = useAuthStore((state) => state.identity);
  const [actionError, setActionError] = useState<string | null>(null);
  const latestRoom = useRef<RoomView | null>(null);
  const leaving = useRef(false);
  const pendingLeave = useRef<number | null>(null);
  const enteringMatch = useRef(false);
  const roomUnavailable = useRef(false);
  const room = useQuery({
    queryKey: ["room", roomId, identity?.id],
    queryFn: async () => {
      const current = await gameApi.getRoom(roomId, token!);
      if (
        identity &&
        !current.members.some((member) => member.user_id === identity.id)
      ) {
        try {
          return await gameApi.joinRoom(roomId, current.version, token!);
        } catch (error: unknown) {
          if (apiFailure(error).code === "lobby.user_busy") {
            const activity = await gameApi.activity(token!);
            if (activity.kind === "room" && activity.room_id) {
              const prevRoom = await gameApi.getRoom(
                activity.room_id,
                token!,
              );
              await gameApi.leaveRoom(
                activity.room_id,
                prevRoom.version,
                token!,
              );
              const updated = await gameApi.getRoom(roomId, token!);
              return await gameApi.joinRoom(
                roomId,
                updated.version,
                token!,
              );
            }
            await resumeCurrentActivity(token!);
          }
          throw error;
        }
      }
      return current;
    },
    enabled: Boolean(token && identity),
    /* 本来就两秒一拉，缓存的新旧没有意义；进房当场重拉一份，早点认准房态。 */
    staleTime: 0,
    retry: (failureCount, error) =>
      !(error instanceof ApiError && error.status === 404) &&
      failureCount < 2,
    refetchInterval: (query) => (query.state.error ? false : 2000),
  });
  useSceneReady(!room.isLoading);

  useEffect(() => {
    latestRoom.current = room.data ?? null;
  }, [room.data]);

  useEffect(() => {
    if (pendingLeave.current !== null) {
      window.clearTimeout(pendingLeave.current);
      pendingLeave.current = null;
    }
    leaving.current = false;
    const leave = () => {
      const current = latestRoom.current;
      if (
        leaving.current ||
        enteringMatch.current ||
        roomUnavailable.current ||
        !token ||
        !identity ||
        !current ||
        current.lifecycle !== "waiting" ||
        !current.members.some((member) => member.user_id === identity.id)
      ) {
        return;
      }
      leaving.current = true;
      gameApi.leaveRoomOnExit(roomId, token);
    };
    const leaveAfterUnmount = () => {
      pendingLeave.current = window.setTimeout(() => {
        pendingLeave.current = null;
        leave();
      }, 0);
    };
    window.addEventListener("pagehide", leave);
    return () => {
      window.removeEventListener("pagehide", leave);
      leaveAfterUnmount();
    };
  }, [identity, roomId, token]);

  const activeMatchId = matchToEnter(room.data, room.isFetchedAfterMount);

  useEffect(() => {
    if (activeMatchId) {
      enteringMatch.current = true;
      navigateTo({ kind: "game", matchId: activeMatchId });
    }
  }, [activeMatchId]);

  const roomMissing =
    room.error instanceof ApiError && room.error.status === 404;

  useEffect(() => {
    if (!roomMissing || !token) return;
    roomUnavailable.current = true;
    navigateTo({ kind: "lobby" });
  }, [roomMissing, token]);

  if (room.isLoading) {
    return <div className="room-screen__loading">加载中…</div>;
  }
  if (room.error) {
    return (
      <div className="room-screen__error">
        <p>{roomMissing ? "房间已结束，正在返回大厅…" : apiFailure(room.error).message}</p>
        <button type="button" onClick={() => navigateTo({ kind: "lobby" })}>
          返回大厅
        </button>
      </div>
    );
  }
  if (!room.data) return null;

  const data = room.data;
  const isOwner = data.owner_user_id === identity?.id;
  const isMember = data.members.some(
    (member) => member.user_id === identity?.id,
  );
  const selfMember = data.members.find(
    (member) => member.user_id === identity?.id,
  );
  const allReady =
    data.members.length === data.seat_count &&
    data.members.every((member) => member.ready);
  const membersBySeat = new Map(
    data.members.map((member) => [member.seat, member]),
  );

  if (activeMatchId) {
    return null;
  }

  const call =
    (action: () => Promise<unknown>, errorMessage: string) => async () => {
      setActionError(null);
      try {
        await action();
        await room.refetch();
      } catch {
        setActionError(errorMessage);
      }
    };

  return (
    <section className="room-screen" aria-label="好友房间">
      <div
        className="room-screen__background"
        style={{ backgroundImage: `url("${roomBackground}")` }}
        aria-hidden="true"
      />
      <div className="room-screen__veil" aria-hidden="true" />

      <div className="room-screen__content">
        <header className="room-screen__header">
          <div>
            {/*
              大字原来写房间名，可房间名多半是默认的「好友房间」，占着最大的字号
              什么也没说。挪到上面那个小标签里，大字腾给这桌打的是什么——种类、
              人数长度（冲击麻将则是模式）、规则名，进房的人不用去翻建房设置。
            */}
            <span className="room-screen__eyebrow">{data.name}</span>
            <h1>{roomRuleTitle(data)}</h1>
          </div>
          <span className="room-screen__number">
            <small>房间号</small>
            {/* 房间号要发给朋友，留一处可以选中复制。 */}
            <strong className="is-selectable">{data.id}</strong>
          </span>
        </header>

        <div
          className={`room-player-grid room-player-grid--${data.seat_count}`}
        >
          {seatNames.slice(0, data.seat_count).map((seatName, seat) => (
            <PlayerCard
              key={seatName}
              seatName={seatName}
              member={membersBySeat.get(seat)}
              ownerUserId={data.owner_user_id}
            />
          ))}
        </div>

        <footer className="room-screen__footer">
          <div className="room-screen__message" role="status">
            {actionError}
          </div>
          <div className="room-screen__actions">
            {isMember && !selfMember?.ready && identity && (
              <RoomAction
                label="角色设置"
                onClick={() => {
                  enteringMatch.current = true;
                  navigateTo({
                    kind: "profile",
                    userId: identity.id,
                    tab: "character",
                    returnRoomId: roomId,
                  });
                }}
              />
            )}
            {isMember && (
              <RoomAction
                label={selfMember?.ready ? "取消准备" : "准备"}
                emphasis
                onClick={call(
                  () =>
                    gameApi.setReady(
                      roomId,
                      data.version,
                      !selfMember?.ready,
                      token!,
                    ),
                  "准备状态更新失败",
                )}
              />
            )}
            {isOwner && (
              <RoomAction
                label="开始对局"
                emphasis
                disabled={!allReady}
                onClick={call(
                  () => gameApi.startRoom(roomId, data.version, token!),
                  "开始对局失败",
                )}
              />
            )}
            <RoomAction
              label="返回大厅"
              onClick={() => navigateTo({ kind: "lobby" })}
            />
          </div>
        </footer>
      </div>
    </section>
  );
}

function PlayerCard({
  seatName,
  member,
  ownerUserId,
}: {
  seatName: string;
  member?: RoomMemberView;
  ownerUserId: string;
}) {
  if (!member) {
    return (
      <article className="room-player-card room-player-card--empty">
        <span className="room-player-card__seat">{seatName}</span>
        <div className="room-player-card__empty-mark">待</div>
        <div className="room-player-card__info">
          <strong>等待加入</strong>
        </div>
      </article>
    );
  }

  const isOwner = member.user_id === ownerUserId;
  return (
    <article
      className={`room-player-card${member.ready ? " is-ready" : ""}`}
    >
      <span className="room-player-card__seat">{seatName}</span>
      {isOwner && <span className="room-player-card__owner">房主</span>}
      {member.ready && (
        <span className="room-player-card__ready">已准备</span>
      )}
      <div className="room-player-card__illustration">
        <img
          src={member.character.illustration_path}
          alt={member.character.name}
        />
      </div>
      <div className="room-player-card__info">
        <strong>{member.nickname}</strong>
      </div>
    </article>
  );
}

function RoomAction({
  label,
  onClick,
  emphasis = false,
  disabled = false,
}: {
  label: string;
  onClick: () => void;
  emphasis?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={`room-screen__action${emphasis ? " is-emphasis" : ""}`}
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </button>
  );
}
