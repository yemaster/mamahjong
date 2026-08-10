/**
 * 观察者视图补丁的应用。
 *
 * 服务端按连接记着上一次发出去的视图，之后只发它和新视图之间的差；这里把差
 * 打回去还原出新视图。补丁只描述差异、不描述语义，因此这一层完全不认识麻将，
 * 加字段、改规则都不用动它。
 *
 * 协议见 `docs/realtime-transport.md`。
 */

/** 补丁树的一个节点，三选一。 */
export type ViewPatch =
  /** 整个子树换成这个值。 */
  | { set: unknown }
  /** 对象逐键；`del` 是被删掉的键。 */
  | { obj?: Record<string, ViewPatch>; del?: string[] }
  /** 数组：`len` 截断、`at` 逐个下标求差、`push` 追加。 */
  | {
      arr: {
        len?: number;
        at?: Record<string, ViewPatch>;
        push?: unknown[];
      };
    };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * 把 `patch` 打在 `value` 上，返回新值。
 *
 * 不改动传进来的对象：界面靠引用变化判断该不该重画，原地改会让整块牌桌漏更新。
 */
export function applyViewPatch(value: unknown, patch: unknown): unknown {
  if (!isRecord(patch)) return value;

  if ("set" in patch) return patch.set;

  if (isRecord(patch.arr)) {
    const { len, at, push } = patch.arr as {
      len?: unknown;
      at?: unknown;
      push?: unknown;
    };
    const items = Array.isArray(value) ? [...value] : [];
    if (typeof len === "number") items.length = len;
    if (isRecord(at)) {
      for (const [index, child] of Object.entries(at)) {
        const slot = Number(index);
        items[slot] = applyViewPatch(items[slot], child);
      }
    }
    if (Array.isArray(push)) items.push(...push);
    return items;
  }

  const fields: Record<string, unknown> = isRecord(value) ? { ...value } : {};
  if (Array.isArray(patch.del)) {
    for (const key of patch.del) delete fields[key as string];
  }
  if (isRecord(patch.obj)) {
    for (const [key, child] of Object.entries(patch.obj)) {
      fields[key] = applyViewPatch(fields[key], child);
    }
  }
  return fields;
}
