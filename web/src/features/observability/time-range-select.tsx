import { Clock3Icon } from "lucide-react";
import {
  Select,
  SelectTrigger,
  SelectContent,
  SelectItem,
  SelectValue,
} from "@/components/ui/select";

const labels: Record<string, string> = {
  "15m": "Last 15 minutes",
  "1h": "Last hour",
  "6h": "Last 6 hours",
  "24h": "Last 24 hours",
  "7d": "Last 7 days",
  "30d": "Last 30 days",
  custom: "Custom range",
};

export function TimeRangeSelect({
  value,
  label,
  timezone,
  onChange,
}: {
  value: string;
  label: string;
  timezone: string;
  onChange: (value: string) => void;
}) {
  return (
    <Select
      value={value}
      onValueChange={(next) => {
        if (next) onChange(next);
      }}
    >
      <SelectTrigger
        size="sm"
        aria-label={label}
        title={`Newest first · ${timezone}`}
        className="trace-controls h-[26px] gap-2 rounded-none border-border bg-background px-2 text-[11px] shadow-none hover:bg-muted data-[size=sm]:rounded-none data-popup-open:bg-muted"
      >
        <Clock3Icon className="size-3 text-muted-foreground" />
        <SelectValue>{labels[value]}</SelectValue>
      </SelectTrigger>
      <SelectContent
        className="trace-controls min-w-48 rounded-none border border-border bg-popover p-0 shadow-lg ring-0"
        alignItemWithTrigger={false}
        side="bottom"
        sideOffset={4}
        align="end"
      >
        {Object.entries(labels).map(([key, text]) => (
          <SelectItem
            key={key}
            value={key}
            className="h-7 rounded-none border-b border-border/50 pl-2 text-[11px] last:border-b-0 data-selected:bg-muted data-selected:font-semibold"
          >
            {text}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
