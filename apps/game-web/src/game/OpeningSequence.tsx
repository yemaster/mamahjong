export type OpeningPhase = "dice" | "deal" | "waiting" | "play";

export function openingDice(
  matchId: string,
  handIndex: number,
): [number, number] {
  let hash = handIndex + 17;
  for (const character of matchId) {
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  }
  /* 第一个骰子用 hash 直接取模，第二个用 hash 的高位混合低位后取模，保证独立性 */
  const dice1 = (hash % 6) + 1;
  const dice2 = (((hash >>> 16) ^ (hash & 0xFFFF)) % 6) + 1;
  return [dice1, dice2];
}
