import { useEffect, useMemo, useState } from "react";
import type { MatchView } from "../types";

const fallbackAvatar =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

/** 剩几秒开始转红搏动。 */
const URGENT_SECONDS = 5;

/**
 * 好友房的退出投票面板。
 *
 * 和对局其余界面同一套语言：深色底、细金边、游戏字体，按钮沿用操作按钮那道
 * 斜切笔刷。四家一家一行往下排，一眼能看出还差谁没投。
 */
export function ExitVotePanel({
  view,
  onVote,
}: {
  view: MatchView;
  onVote: (agree: boolean) => void;
}) {
  const vote = view.exit_vote;
  const [deadline, setDeadline] = useState(
    () => Date.now() + (vote?.remaining_ms ?? 0),
  );
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (vote) setDeadline(Date.now() + vote.remaining_ms);
  }, [vote?.initiator_seat, vote?.remaining_ms]);

  useEffect(() => {
    if (!vote) return;
    const timer = window.setInterval(() => setNow(Date.now()), 200);
    return () => window.clearInterval(timer);
  }, [vote]);

  const seconds = Math.max(0, Math.ceil((deadline - now) / 1000));
  const ownChoice = vote?.votes[view.observer_seat] ?? null;
  const orderedPlayers = useMemo(
    () =>
      [...view.players].sort(
        (left, right) => left.seat - right.seat,
      ),
    [view.players],
  );

  if (!vote) return null;

  const initiator = view.players.find(
    (player) => player.seat === vote.initiator_seat,
  );

  return (
    <div
      className="match-exit-vote"
      role="dialog"
      aria-modal="true"
      aria-label="退出对战投票"
    >
      <section className="match-exit-vote__panel">
        <header className="match-exit-vote__head">
          <h2>退出对战</h2>
          <strong className={seconds <= URGENT_SECONDS ? "is-urgent" : ""}>
            {seconds}
          </strong>
        </header>
        <p className="match-exit-vote__lead">
          {initiator?.nickname ?? "有人"} 提议结束本局对战
        </p>
        <ul className="match-exit-vote__players">
          {orderedPlayers.map((player) => {
            const choice = vote.votes[player.seat];
            const marks = [
              choice == null ? "is-waiting" : "is-decided",
              player.seat === view.observer_seat ? "is-self" : "",
            ]
              .filter(Boolean)
              .join(" ");
            return (
              <li key={player.user_id} className={marks}>
                <img
                  src={player.avatar_path ?? fallbackAvatar}
                  alt=""
                  onError={(event) => {
                    event.currentTarget.src = fallbackAvatar;
                  }}
                />
                <span>{player.nickname}</span>
                {player.seat === vote.initiator_seat && <em>发起</em>}
                <b
                  className={
                    choice === true
                      ? "is-agree"
                      : choice === false
                        ? "is-disagree"
                        : ""
                  }
                >
                  {choice === true
                    ? "同意"
                    : choice === false
                      ? "不同意"
                      : "等待"}
                </b>
              </li>
            );
          })}
        </ul>
        <div className="match-exit-vote__actions">
          <button
            type="button"
            className="match-brush-button is-quiet"
            disabled={ownChoice != null}
            onClick={() => onVote(false)}
          >
            不同意
          </button>
          <button
            type="button"
            className="match-brush-button is-emphasis"
            disabled={ownChoice != null}
            onClick={() => onVote(true)}
          >
            同意
          </button>
        </div>
      </section>
    </div>
  );
}
