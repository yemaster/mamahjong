import { describe, expect, it } from "vitest";
import { applyViewPatch } from "./viewPatch";

describe("applyViewPatch", () => {
  it("replaces a whole subtree", () => {
    expect(applyViewPatch({ result: null }, { set: { seats: [0, 1] } })).toEqual(
      { seats: [0, 1] },
    );
  });

  it("only touches the keys the patch names", () => {
    const before = {
      version: 3,
      players: [{ seat: 0, nickname: "阿伟", points: 25000 }],
    };
    const after = applyViewPatch(before, {
      obj: {
        version: { set: 4 },
        players: { arr: { at: { "0": { obj: { points: { set: 23000 } } } } } },
      },
    });

    expect(after).toEqual({
      version: 4,
      players: [{ seat: 0, nickname: "阿伟", points: 23000 }],
    });
    // 界面靠引用变化重画，原地改会让整块牌桌漏更新。
    expect(before.version).toBe(3);
    expect(after).not.toBe(before);
  });

  it("appends to a river without resending it", () => {
    expect(
      applyViewPatch({ discards: [{ id: 1 }, { id: 2 }] }, {
        obj: { discards: { arr: { push: [{ id: 9 }] } } },
      }),
    ).toEqual({ discards: [{ id: 1 }, { id: 2 }, { id: 9 }] });
  });

  it("truncates a shrinking array", () => {
    expect(
      applyViewPatch([1, 2, 3, 4], { arr: { len: 2, at: { "1": { set: 9 } } } }),
    ).toEqual([1, 9]);
  });

  it("adds and removes keys", () => {
    expect(
      applyViewPatch(
        { exit_vote: null, gone: 1 },
        {
          obj: { exit_vote: { set: { initiator_seat: 2 } }, fresh: { set: true } },
          del: ["gone"],
        },
      ),
    ).toEqual({ exit_vote: { initiator_seat: 2 }, fresh: true });
  });

  it("reproduces the same view the server holds", () => {
    // 服务端那边的 Rust 测试用的是同一套操作码，两边必须理解一致。
    const before = {
      version: 41,
      phase: { kind: "awaiting_discard", seat: 1 },
      players: [
        { seat: 0, nickname: "阿伟", discards: [{ id: 1 }], furiten: false },
        { seat: 1, nickname: "小林", discards: [], furiten: false },
      ],
      clocks: [{ seat: 0, remaining_ms: 9000 }],
    };
    const after = applyViewPatch(before, {
      obj: {
        version: { set: 42 },
        phase: { obj: { kind: { set: "awaiting_reaction" }, seat: { set: 2 } } },
        players: {
          arr: {
            at: {
              "1": {
                obj: {
                  discards: { arr: { push: [{ id: 7 }] } },
                  furiten: { set: true },
                },
              },
            },
          },
        },
        clocks: { arr: { at: { "0": { obj: { remaining_ms: { set: 4000 } } } } } },
      },
    });

    expect(after).toEqual({
      version: 42,
      phase: { kind: "awaiting_reaction", seat: 2 },
      players: [
        { seat: 0, nickname: "阿伟", discards: [{ id: 1 }], furiten: false },
        { seat: 1, nickname: "小林", discards: [{ id: 7 }], furiten: true },
      ],
      clocks: [{ seat: 0, remaining_ms: 4000 }],
    });
  });

  it("leaves the value alone when the patch is not a node", () => {
    expect(applyViewPatch({ a: 1 }, null)).toEqual({ a: 1 });
  });
});
