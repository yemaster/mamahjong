import { describe, expect, it, vi } from "vitest";
import { completeAdminBatch } from "./batchActions";

describe("completeAdminBatch", () => {
  it("waits for every operation before reporting partial failure", async () => {
    const later = vi.fn().mockResolvedValue(undefined);
    await expect(completeAdminBatch([
      () => Promise.resolve(),
      () => Promise.reject(new Error("不能删除默认项")),
      later,
    ])).rejects.toThrow("2 项成功，1 项失败：不能删除默认项");
    expect(later).toHaveBeenCalledOnce();
  });
});
