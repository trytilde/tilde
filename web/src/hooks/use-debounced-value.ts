import { useEffect, useState } from "react";

/** `value` once it has stopped changing for `delay` ms; search inputs query the server with it. */
export function useDebouncedValue<T>(value: T, delay = 250) {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timeout = setTimeout(() => setSettled(value), delay);
    return () => clearTimeout(timeout);
  }, [value, delay]);
  return settled;
}
