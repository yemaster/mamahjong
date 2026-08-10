import type { TableCameraConfig } from "./table";

export interface TablePerspectiveSettings {
  height: number;
  angle: number;
  fov: number;
  targetY: number;
  targetZ: number;
}

export const defaultTablePerspectiveSettings: TablePerspectiveSettings = {
  height: 21,
  angle: 50,
  fov: 20,
  targetY: 0.15,
  targetZ: 0.7,
};

const STORAGE_PREFIX = "mamahjong_table_camera_v1:";

export function tableCameraConfigFromSettings(
  settings: TablePerspectiveSettings,
): TableCameraConfig {
  const radians = settings.angle * (Math.PI / 180);
  const verticalDistance = settings.height - settings.targetY;
  return {
    mode: "perspective",
    fov: settings.fov,
    orthographicSize: 13.4,
    y: settings.height,
    z:
      settings.targetZ +
      verticalDistance / Math.max(0.01, Math.tan(radians)),
    targetY: settings.targetY,
    targetZ: settings.targetZ,
  };
}

export function loadTablePerspectiveSettings(
  userId: string | null | undefined,
): TablePerspectiveSettings {
  if (!userId) return defaultTablePerspectiveSettings;
  try {
    const raw = localStorage.getItem(`${STORAGE_PREFIX}${userId}`);
    if (!raw) return defaultTablePerspectiveSettings;
    const value = JSON.parse(raw) as Partial<TablePerspectiveSettings>;
    return normalizeTablePerspectiveSettings(value);
  } catch {
    return defaultTablePerspectiveSettings;
  }
}

export function saveTablePerspectiveSettings(
  userId: string,
  settings: TablePerspectiveSettings,
): void {
  try {
    localStorage.setItem(
      `${STORAGE_PREFIX}${userId}`,
      JSON.stringify(normalizeTablePerspectiveSettings(settings)),
    );
  } catch {
    /* 浏览器禁用本地存储时保留本次预览，不阻断返回流程。 */
  }
}

export function normalizeTablePerspectiveSettings(
  value: Partial<TablePerspectiveSettings>,
): TablePerspectiveSettings {
  return {
    height: bounded(value.height, 10, 100, 21),
    angle: bounded(value.angle, 15, 75, 50),
    fov: bounded(value.fov, 2, 20, 20),
    targetY: bounded(value.targetY, -2, 3, 0.15),
    targetZ: bounded(value.targetZ, -5, 5, 0.7),
  };
}

function bounded(
  value: number | undefined,
  min: number,
  max: number,
  fallback: number,
): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(max, Math.max(min, value))
    : fallback;
}
