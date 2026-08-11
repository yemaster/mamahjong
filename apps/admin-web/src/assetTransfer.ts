import type {
  AdminCharacter,
  AdminMusic,
  AdminTablecloth,
  CharacterInput,
  MusicInput,
  TableclothInput,
} from "./types";

export type AssetBundleKind = "characters" | "tablecloths" | "music";

export interface AssetBundle<T> {
  schema: "admin_asset_bundle.v1";
  kind: AssetBundleKind;
  exported_at: string;
  items: T[];
}

type Guard<T> = (value: unknown) => value is T;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}

function isCharacterAsset(value: unknown): boolean {
  return isRecord(value) && isString(value.name) && isString(value.path);
}

function isCharacterOutfit(value: unknown): boolean {
  return isRecord(value) && isString(value.id) && isString(value.name) && isString(value.illustration_path);
}

export const isCharacterInput: Guard<CharacterInput> = (value): value is CharacterInput =>
  isRecord(value) &&
  isString(value.id) &&
  isString(value.name) &&
  isString(value.illustration_path) &&
  Array.isArray(value.emotes) && value.emotes.every(isCharacterAsset) &&
  Array.isArray(value.voices) && value.voices.every(isCharacterAsset) &&
  Array.isArray(value.outfits) && value.outfits.every(isCharacterOutfit) &&
  isBoolean(value.enabled) &&
  isBoolean(value.is_default);

export const isTableclothInput: Guard<TableclothInput> = (value): value is TableclothInput =>
  isRecord(value) &&
  isString(value.id) &&
  isString(value.name) &&
  isString(value.texture_path) &&
  isBoolean(value.enabled) &&
  isBoolean(value.is_default);

export const isMusicInput: Guard<MusicInput> = (value): value is MusicInput =>
  isRecord(value) &&
  isString(value.id) &&
  isString(value.name) &&
  (value.scene === "lobby" || value.scene === "match" || value.scene === "riichi") &&
  isString(value.audio_path) &&
  typeof value.duration_ms === "number" &&
  Number.isSafeInteger(value.duration_ms) &&
  value.duration_ms > 0 &&
  isBoolean(value.enabled) &&
  isBoolean(value.is_default);

export function characterInput(item: AdminCharacter): CharacterInput {
  const { version: _version, ...input } = item;
  return input;
}

export function tableclothInput(item: AdminTablecloth): TableclothInput {
  const { version: _version, ...input } = item;
  return input;
}

export function musicInput(item: AdminMusic): MusicInput {
  const { version: _version, ...input } = item;
  return input;
}

export function createAssetBundle<T>(kind: AssetBundleKind, items: T[], now = new Date()): AssetBundle<T> {
  return { schema: "admin_asset_bundle.v1", kind, exported_at: now.toISOString(), items };
}

export function parseAssetBundle<T>(source: string, kind: AssetBundleKind, guard: Guard<T>): T[] {
  let value: unknown;
  try {
    value = JSON.parse(source);
  } catch {
    throw new Error("文件不是有效的 JSON");
  }
  if (!isRecord(value) || value.schema !== "admin_asset_bundle.v1" || value.kind !== kind || !Array.isArray(value.items)) {
    throw new Error("文件类型或版本不正确");
  }
  if (!value.items.length) throw new Error("文件中没有可导入的数据");
  if (!value.items.every(guard)) throw new Error("文件中的数据格式不正确");
  return value.items;
}

export async function readAssetBundle<T>(file: File, kind: AssetBundleKind, guard: Guard<T>): Promise<T[]> {
  return parseAssetBundle(await file.text(), kind, guard);
}

export async function upsertAssetItems<T extends { id: string }>(
  items: T[],
  existingIds: Iterable<string>,
  create: (item: T) => Promise<unknown>,
  update: (item: T) => Promise<unknown>,
) {
  const existing = new Set(existingIds);
  for (const item of items) {
    if (existing.has(item.id)) await update(item);
    else {
      await create(item);
      existing.add(item.id);
    }
  }
}

export function downloadAssetBundle<T>(kind: AssetBundleKind, items: T[]) {
  const bundle = createAssetBundle(kind, items);
  const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `mamahjong-${kind}-${bundle.exported_at.slice(0, 10)}.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}
