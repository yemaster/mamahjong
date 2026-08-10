export type TileArtwork = "jp" | "cn";

export const tileCodes = [
  "0m",
  "1m",
  "2m",
  "3m",
  "4m",
  "5m",
  "6m",
  "7m",
  "8m",
  "9m",
  "0p",
  "1p",
  "2p",
  "3p",
  "4p",
  "5p",
  "6p",
  "7p",
  "8p",
  "9p",
  "0s",
  "1s",
  "2s",
  "3s",
  "4s",
  "5s",
  "6s",
  "7s",
  "8s",
  "9s",
  "1z",
  "2z",
  "3z",
  "4z",
  "5z",
  "6z",
  "7z",
] as const;

export function normalizeTileCode(code: string): string {
  const match = /^([0-9])([mpsz])(r)?$/.exec(code.trim().toLowerCase());
  if (!match) return code;
  const [, number, suit, red] = match;
  return red ? `0${suit}` : `${number}${suit}`;
}

export function tileAssetPath(
  code: string,
  _artwork: TileArtwork = "jp",
): string {
  const normalized = normalizeTileCode(code);
  return `${import.meta.env.BASE_URL}assets/local-game-assets/mahjong-graphic/vectors/${normalized}.svg?v=3e275804`;
}

export function tileBackAssetPath(
  artwork: TileArtwork = "jp",
): string {
  return `${import.meta.env.BASE_URL}assets/local-game-assets/mahjim/tiles-fixed/${artwork}/back.png?v=20260804-layout2`;
}

export function doraCodeForIndicator(code: string): string {
  const normalized = normalizeTileCode(code);
  const suit = normalized.slice(-1);
  const rawNumber = Number(normalized.slice(0, -1));
  const number = rawNumber === 0 ? 5 : rawNumber;
  if (suit === "z") {
    if (number >= 1 && number <= 4) return `${number === 4 ? 1 : number + 1}z`;
    if (number >= 5 && number <= 7) return `${number === 7 ? 5 : number + 1}z`;
    return normalized;
  }
  if (suit === "m" || suit === "p" || suit === "s") {
    return `${number === 9 ? 1 : number + 1}${suit}`;
  }
  return normalized;
}

export function isDoraTile(
  code: string,
  indicators: { code: string }[],
): boolean {
  const normalized = normalizeTileCode(code);
  if (/^0[mps]$/.test(normalized)) return true;
  const comparable =
    normalized.startsWith("0") ? `5${normalized.slice(-1)}` : normalized;
  return indicators.some(
    (indicator) => doraCodeForIndicator(indicator.code) === comparable,
  );
}
