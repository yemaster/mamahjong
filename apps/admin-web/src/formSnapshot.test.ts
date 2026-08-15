import { reactive } from "vue";
import { describe, expect, it } from "vitest";
import { formSnapshot } from "./formSnapshot";

describe("formSnapshot", () => {
  it("clones Vue reactive models into plain request payloads", () => {
    const model = reactive({ name: "大厅音乐", nested: [{ path: "/user-assets/music/lobby.ogg" }] });

    const snapshot = formSnapshot(model);

    expect(snapshot).toEqual(model);
    expect(snapshot).not.toBe(model);
    expect(snapshot.nested).not.toBe(model.nested);
  });
});
