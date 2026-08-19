import type { MahjongFamily } from "../types";
import { normalizeTileCode, tileCodes } from "./tileAssets";

/** 开发模式按键数量：轮到自己的暗手最多 14 张（13 张 + 刚摸的那张）。 */
export const DEV_HAND_SIZE = 14;

/**
 * 依次对应第 1..14 张牌的按键，QWERTY 顶行接 home 行：
 * q w e r t y u i o p a s d f。
 */
export const DEV_HAND_KEYS = "qwertyuiopasdf";

/**
 * 开发模式只能由构建期环境变量打开：`scripts/dev.sh` 把 `MAMAHJONG_DEV_MODE`
 * 经 compose build-arg 传进 Vite 的 `VITE_MAMAHJONG_DEV_MODE`。其它地方一律
 * 不设，所以默认关闭。
 */
export function isDevModeEnabled(): boolean {
  return import.meta.env.VITE_MAMAHJONG_DEV_MODE === "true";
}

/**
 * 当前牌山（规则家族）里真实存在的牌码，改牌只在这份里循环。
 *
 * - 冲击麻将没有赤牌：跳过 0m/0p/0s。
 * - 四川麻将只有三门数牌：跳过字牌和赤牌。
 * - 立直三麻没有 2m..8m，也没有赤 5m：跳过 2m..8m 和 0m。
 * - 立直四麻全量。
 */
export function validTileCodes(
  variantKind: MahjongFamily,
  sanma: boolean,
): string[] {
  if (variantKind === "impact") {
    return tileCodes.filter((code) => !code.startsWith("0"));
  }
  if (variantKind === "sichuan") {
    return tileCodes.filter((code) => /^[1-9][mps]$/.test(code));
  }
  if (sanma) {
    return tileCodes.filter(
      (code) => code !== "0m" && !/^[2-8]m$/.test(code),
    );
  }
  return [...tileCodes];
}

/**
 * 把一张牌推进给定牌码循环里的下一张。牌码不在这份循环里的（比如副露后按到超出
 * 手牌范围的键，或这张牌根本不属于当前牌山）原样返回，不瞎猜。
 */
export function advanceTileCode(
  code: string,
  validCodes: readonly string[],
): string {
  const normalized = normalizeTileCode(code);
  const index = validCodes.indexOf(normalized);
  if (index < 0) return normalized;
  return validCodes[(index + 1) % validCodes.length]!;
}
