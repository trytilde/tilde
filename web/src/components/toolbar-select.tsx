import type { ReactNode } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

/**
 * A compact filter dropdown sized to its label, styled like the tracing and logs toolbars.
 * Belongs inside a `trace-controls` row.
 */
export function ToolbarSelect<T extends string>({
  label,
  value,
  options,
  onChange,
  icon,
  prefix,
}: {
  /** Accessible name. */
  label: string;
  value: T;
  /** `hint`: a muted second line in the menu. */
  options: { value: T; label: string; hint?: string; disabled?: boolean }[];
  onChange: (value: T) => void;
  icon?: ReactNode;
  /** A muted label before the value (`Environment`), hidden on narrow screens. */
  prefix?: string;
}) {
  return (
    <Select
      value={value}
      onValueChange={(next) => {
        if (next !== null) onChange(next as T);
      }}
    >
      <SelectTrigger
        size="sm"
        aria-label={label}
        className="trace-controls h-[26px] w-auto gap-1.5 rounded-md border-input bg-background px-2 text-[11px] shadow-none hover:bg-muted data-[size=sm]:rounded-md data-popup-open:bg-muted [&_svg:not([class*='size-'])]:size-3"
      >
        {icon}
        {prefix && <span className="hidden text-muted-foreground sm:inline">{prefix}</span>}
        <SelectValue>{options.find((option) => option.value === value)?.label}</SelectValue>
      </SelectTrigger>
      <SelectContent
        className="trace-controls w-auto min-w-40 rounded-md border border-border bg-popover p-0 shadow-lg ring-0"
        alignItemWithTrigger={false}
        side="bottom"
        sideOffset={4}
        align="end"
      >
        {options.map((option) => (
          <SelectItem
            key={option.value}
            value={option.value}
            disabled={option.disabled}
            className="h-auto min-h-7 rounded-none border-b border-border/50 pl-2 text-[11px] last:border-b-0 data-selected:bg-muted data-selected:font-semibold"
          >
            {option.hint ? (
              <span className="grid py-1">
                {option.label}
                <span className="font-normal text-muted-foreground">{option.hint}</span>
              </span>
            ) : (
              option.label
            )}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
