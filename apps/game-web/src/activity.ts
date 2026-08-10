import { gameApi } from "./api";
import { navigateTo } from "./routing";
import type { UserActivity } from "./types";

export async function resumeCurrentActivity(token: string): Promise<boolean> {
  const activity = await gameApi.activity(token);
  return navigateToActivity(activity);
}

export function navigateToActivity(activity: UserActivity): boolean {
  if (activity.kind === "game" && activity.match_id) {
    navigateTo({ kind: "game", matchId: activity.match_id });
    return true;
  }
  if (activity.kind === "room" && activity.room_id) {
    navigateTo({ kind: "room", roomId: activity.room_id });
    return true;
  }
  if (activity.kind === "matchmaking" && activity.ticket_id) {
    navigateTo({
      kind: "matchmaking",
      ticketId: activity.ticket_id,
    });
    return true;
  }
  return false;
}
