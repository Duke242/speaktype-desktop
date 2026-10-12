/** Time-based attack/release smoothing, independent of event and frame rates. */
export function smoothLevel(current: number, target: number, elapsedMs: number) {
  const level = Number.isFinite(target) ? Math.max(0, Math.min(1, target)) : 0;
  const timeConstant = level > current ? 70 : 220;
  return current + (level - current) * (1 - Math.exp(-Math.max(0, elapsedMs) / timeConstant));
}
