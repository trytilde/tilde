// Explicit formats keep tracing grids comparable across rows and browser locales.
const counts = new Intl.NumberFormat("en-GB", {
  minimumFractionDigits: 0,
  maximumFractionDigits: 0,
});
const secondsFormat = new Intl.NumberFormat("en-GB", {
  minimumFractionDigits: 3,
  maximumFractionDigits: 3,
});
const dollars = new Intl.NumberFormat("en-GB", {
  minimumFractionDigits: 6,
  maximumFractionDigits: 6,
});
export const number = (value?: number | bigint) =>
  value === undefined || (typeof value === "number" && !Number.isFinite(value))
    ? "—"
    : counts.format(value);
/** Source durations and display values use seconds; labels declare the unit. */
export const duration = (seconds?: number) =>
  seconds === undefined || !Number.isFinite(seconds) ? "—" : secondsFormat.format(seconds);
/** USD units belong in the column/field label, rather than each numeric cell. */
export const cost = (value?: number) =>
  value === undefined || !Number.isFinite(value) ? "—" : dollars.format(value);
/** DD/MM/YY HH:mm:ss in the browser's local timezone, always using a 24-hour clock. */
export function date(value: string) {
  const time = new Date(value);
  if (!Number.isFinite(time.getTime())) return "—";
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${pad(time.getDate())}/${pad(time.getMonth() + 1)}/${pad(time.getFullYear() % 100)} ${pad(time.getHours())}:${pad(time.getMinutes())}:${pad(time.getSeconds())}`;
}
