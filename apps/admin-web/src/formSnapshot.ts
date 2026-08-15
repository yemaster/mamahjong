import { toRaw } from "vue";

/** Creates a plain request payload from a Vue reactive form model. */
export function formSnapshot<T>(model: T): T {
  return structuredClone(toRaw(model));
}
