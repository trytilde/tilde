// Adapted from Langfuse web/src/utils/numbers.ts; see LANGFUSE-LICENSE.
const divisors = [1, 1000, 60000, 3600000, 86400000];
const units = ["millisecond", "second", "minute", "hour", "day"];
export function duration(seconds?: number) {
  if (seconds === undefined) return "—";
  const ms = seconds * 1000;
  const tier = divisors.reduce(
    (tier, divisor, index) => (Math.abs(ms) >= divisor ? index : tier),
    0,
  );
  return new Intl.NumberFormat("en-US", {
    style: "unit",
    unit: units[tier],
    unitDisplay: "narrow",
    maximumFractionDigits: 2,
  }).format(ms / divisors[tier]);
}
export const cost = (value?: number) =>
  value === undefined
    ? "—"
    : new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: "USD",
        minimumFractionDigits: 2,
        maximumFractionDigits: 6,
      }).format(value);
export const number = (value?: number) => (value === undefined ? "—" : value.toLocaleString());
export const date = (value: string) => (value ? new Date(value).toLocaleString() : "—");
