import type { RoomView } from "../../types";

/**
 * 房间页这一次进来之后，该不该把玩家送进对局。
 *
 * 只认真正拉回来的房态。react-query 缓存里那份是上一局**开局那一刻**写下的，
 * `active_match_id` 还指着刚刚结束的对局；照它跳会把人送回牌桌，牌桌看见
 * `terminated_by_exit_vote` 再把人送回房间，来回两趟在眼里就是接连闪好几下的
 * 转场。缓存那份照样拿来先把界面画出来，只是不拿来跳转。
 */
export function matchToEnter(
  room: RoomView | undefined,
  fetchedAfterMount: boolean,
): string | null {
  if (!fetchedAfterMount) {
    return null;
  }
  return room?.active_match_id ?? null;
}
