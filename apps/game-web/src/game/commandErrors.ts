/**
 * 服务端拒掉一条对局指令时，摆到牌桌上的那句话。
 *
 * 服务端的 message 是给日志看的英文，直接甩给玩家没有意义；这里按错误码换成
 * 一句中文。认不出来的码也要给一句话——最怕的是点了退出什么都不发生，玩家分不清
 * 是没点上还是被拒了。
 */
export function commandRejectionText(code: string): string {
  switch (code) {
    case "game.stale_version":
      return "牌桌刚有新动作，请再试一次";
    case "game.invalid_command":
      return "这个操作现在不能用";
    case "game.finished":
      return "这局已经结束了";
    case "game.not_player":
      return "你不在这局对战里";
    default:
      return "操作没有生效，请再试一次";
  }
}
