import { useState } from "react";
import { format } from "date-fns";
import { enGB } from "date-fns/locale";
import { ChevronDownIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

/** shadcn's date/time composition, using local wall time until the range is applied. */
export function TraceDateTimePicker({
  value,
  onChange,
  label,
  timezone,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
  timezone: string;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const day = value.slice(0, 10);
  const date = day ? new Date(`${day}T00:00:00`) : undefined;
  const time = value.slice(11);
  return (
    <div className="flex min-w-0 items-center gap-1">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger
          render={
            <Button
              type="button"
              variant="outline"
              disabled={disabled}
              aria-label={`${label} date (${timezone})`}
              className="h-[26px] w-[100px] justify-between gap-1 px-2 text-[11px] font-normal"
            >
              {date ? format(date, "dd/MM/yy") : "Select date"}
              <ChevronDownIcon className="size-3" />
            </Button>
          }
        />
        <PopoverContent
          className="trace-controls w-auto overflow-hidden p-0"
          align="start"
          aria-label={`${label} date`}
        >
          <Calendar
            mode="single"
            required
            selected={date}
            captionLayout="dropdown"
            defaultMonth={date}
            locale={enGB}
            className="[--cell-size:--spacing(6)]"
            onSelect={(next) => {
              onChange(`${format(next, "yyyy-MM-dd")}T${time || "00:00:00"}`);
              setOpen(false);
            }}
          />
        </PopoverContent>
      </Popover>
      <Input
        type="time"
        step="1"
        lang="en-GB"
        value={time}
        disabled={disabled}
        aria-label={`${label} time (${timezone})`}
        onChange={(event) => onChange(`${day}T${event.currentTarget.value}`)}
        className="h-[26px] w-[100px] min-w-0 appearance-none bg-background px-2 text-[11px] [&::-webkit-calendar-picker-indicator]:hidden [&::-webkit-calendar-picker-indicator]:appearance-none"
      />
    </div>
  );
}
