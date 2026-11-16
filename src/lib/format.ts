// Pure formatting helpers for fixed-point market values. No Solid, no backend.

const NANOS_PER_SECOND = 1_000_000_000n;

/** Fixed-point integer to a display number at the given decimal scale. */
export function fixedToNumber(value: bigint, scale: number): number {
  return Number(value) / 10 ** scale;
}

/** Unix nanoseconds to whole UTC seconds, the chart engine's time unit. */
export function nanosToSeconds(nanos: bigint): number {
  return Number(nanos / NANOS_PER_SECOND);
}

/** Decimal places a minimum increment implies at its scale. */
export function incrementDecimals(increment: bigint | null, scale: number): number {
  if (increment === null || increment <= 0n) {
    return scale;
  }
  let zeros = 0;
  let rest = increment;
  while (rest % 10n === 0n && zeros < scale) {
    rest /= 10n;
    zeros += 1;
  }
  return scale - zeros;
}

/** Formats a fixed-point value with exactly `decimals` places, no grouping. */
export function formatFixed(value: bigint, scale: number, decimals: number): string {
  const negative = value < 0n;
  const digits = (negative ? -value : value).toString().padStart(scale + 1, "0");
  const whole = digits.slice(0, digits.length - scale) || "0";
  const fraction = scale === 0 ? "" : digits.slice(digits.length - scale);
  const shown = decimals === 0 ? whole : `${whole}.${fraction.slice(0, decimals)}`;
  return negative ? `-${shown}` : shown;
}
