import { describe, expect, it } from "vitest";
import { websocketUrl } from "./ws";

describe("WebSocket 地址", () => {
  it("HTTPS 页面使用安全的 WSS 连接", () => {
    expect(websocketUrl("https://mmj.yemaster.cn", "ticket-1")).toBe(
      "wss://mmj.yemaster.cn/api/v1/ws?ticket=ticket-1",
    );
  });

  it("本地 HTTP 开发环境继续使用 WS 连接", () => {
    expect(websocketUrl("http://127.0.0.1:5173", "ticket 2")).toBe(
      "ws://127.0.0.1:5173/api/v1/ws?ticket=ticket+2",
    );
  });
});
