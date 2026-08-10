import { describe, expect, it } from "vitest";
import { commandRejectionText } from "./commandErrors";

describe("指令被拒的提示", () => {
  it("认识的错误码换成中文", () => {
    expect(commandRejectionText("game.stale_version")).toBe(
      "牌桌刚有新动作，请再试一次",
    );
    expect(commandRejectionText("game.invalid_command")).toBe(
      "这个操作现在不能用",
    );
  });

  it("认不出来的码也要给一句话，不能什么都不说", () => {
    expect(commandRejectionText("server.internal")).not.toBe("");
    expect(commandRejectionText("")).not.toBe("");
  });
});
