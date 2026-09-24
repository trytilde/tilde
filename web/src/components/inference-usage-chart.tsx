import { useEffect, useMemo, useState } from "react";
import { CartesianGrid, Line, LineChart, XAxis, YAxis } from "recharts";
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { timestampDate, timestampFromDate } from "@bufbuild/protobuf/wkt";
import type { GetUsageResponse } from "@trytilde/contracts/tilde/management/v1/inference_pb.js";
import { inference } from "@/client";
import { Button } from "./ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "./ui/chart";

/** Validated categorical palette (dataviz reference instance), fixed order, light and dark steps. */
const SERIES: { light: string; dark: string }[] = [
  { light: "#2a78d6", dark: "#3987e5" },
  { light: "#eb6834", dark: "#d95926" },
  { light: "#1baf7a", dark: "#199e70" },
  { light: "#eda100", dark: "#c98500" },
  { light: "#e87ba4", dark: "#d55181" },
  { light: "#008300", dark: "#008300" },
  { light: "#4a3aa7", dark: "#9085e9" },
  { light: "#e34948", dark: "#e66767" },
];
export const dollars = (micros: bigint | number | undefined) =>
  `$${(Number(micros ?? 0) / 1_000_000).toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
const monthStart = (year: number, month: number) => new Date(Date.UTC(year, month, 1));
const monthLabel = (year: number, month: number) =>
  monthStart(year, month).toLocaleDateString("en-US", {
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  });

/** Daily spend per connection for one month, with total and month-to-date spend beside the title. */
export function InferenceUsageChart({
  agentId,
  names,
  refreshKey = 0,
}: {
  agentId: string;
  /** connection id -> label; connections not listed are shown by id. */
  names: Record<string, string>;
  refreshKey?: number;
}) {
  const now = new Date();
  const [view, setView] = useState({ year: now.getUTCFullYear(), month: now.getUTCMonth() });
  const [usage, setUsage] = useState<GetUsageResponse | null>(null);
  const [current, setCurrent] = useState<GetUsageResponse | null>(null);
  const [error, setError] = useState("");
  const isCurrent = view.year === now.getUTCFullYear() && view.month === now.getUTCMonth();
  useEffect(() => {
    const abort = new AbortController();
    const since = monthStart(view.year, view.month);
    const until = isCurrent ? new Date() : monthStart(view.year, view.month + 1);
    setError("");
    void Promise.all([
      inference.getUsage(
        {
          agentId,
          since: timestampFromDate(since),
          until: timestampFromDate(until),
          bucket: "day",
        },
        { signal: abort.signal },
      ),
      isCurrent
        ? null
        : inference.getUsage(
            {
              agentId,
              since: timestampFromDate(monthStart(now.getUTCFullYear(), now.getUTCMonth())),
              bucket: "day",
            },
            { signal: abort.signal },
          ),
    ])
      .then(([viewed, thisMonth]) => {
        if (abort.signal.aborted) return;
        setUsage(viewed);
        setCurrent(thisMonth ?? viewed);
      })
      .catch((e: unknown) => {
        if (!abort.signal.aborted) setError(e instanceof Error ? e.message : "Usage unavailable");
      });
    return () => abort.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [agentId, view.year, view.month, isCurrent, refreshKey]);

  const { data, config, keys } = useMemo(() => {
    const ids = Array.from(new Set((usage?.series ?? []).map((p) => p.connectionId))).sort();
    const days = isCurrent
      ? now.getUTCDate()
      : new Date(Date.UTC(view.year, view.month + 1, 0)).getUTCDate();
    const rows = Array.from({ length: days }, (_, i) => {
      const day = new Date(Date.UTC(view.year, view.month, i + 1));
      const row: Record<string, number | string> = { date: day.toISOString() };
      for (const id of ids) row[id] = 0;
      return row;
    });
    for (const point of usage?.series ?? []) {
      if (!point.bucketStart) continue;
      const index = timestampDate(point.bucketStart).getUTCDate() - 1;
      if (rows[index]) rows[index][point.connectionId] = Number(point.costMicros ?? 0) / 1_000_000;
    }
    const config: ChartConfig = {};
    ids.forEach((id, i) => {
      const slot = SERIES[i % SERIES.length];
      config[id] = {
        label: names[id] ?? id.slice(0, 8),
        theme: { light: slot.light, dark: slot.dark },
      };
    });
    return { data: rows, config, keys: ids };
  }, [usage, names, view.year, view.month, isCurrent, now]);
  const monthToDate = (current?.connections ?? []).reduce(
    (sum, c) => sum + Number(c.costMicros ?? 0),
    0,
  );

  return (
    <Card className="py-4 sm:py-0">
      <CardHeader className="flex flex-col items-stretch border-b p-0! sm:flex-row">
        <div className="flex flex-1 flex-col justify-center gap-1 px-6 py-4">
          <CardTitle>Inference spend</CardTitle>
          <CardDescription className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Previous month"
              onClick={() =>
                setView((v) =>
                  v.month === 0 ? { year: v.year - 1, month: 11 } : { ...v, month: v.month - 1 },
                )
              }
            >
              <ChevronLeftIcon />
            </Button>
            <span className="min-w-36 text-center">{monthLabel(view.year, view.month)}</span>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Next month"
              disabled={isCurrent}
              onClick={() =>
                setView((v) =>
                  v.month === 11 ? { year: v.year + 1, month: 0 } : { ...v, month: v.month + 1 },
                )
              }
            >
              <ChevronRightIcon />
            </Button>
          </CardDescription>
        </div>
        <CardAction className="flex self-stretch">
          {[
            { label: "Total spend", value: usage?.totalCostMicros },
            { label: "MTD spend", value: monthToDate },
          ].map((metric) => (
            <div
              key={metric.label}
              className="flex flex-1 flex-col justify-center gap-1 border-t px-6 py-4 text-left even:border-l sm:border-t-0 sm:border-l sm:px-8 sm:py-6"
            >
              <span className="text-xs text-muted-foreground">{metric.label}</span>
              <span className="text-lg leading-none font-bold tabular-nums sm:text-2xl">
                {dollars(metric.value)}
              </span>
            </div>
          ))}
        </CardAction>
      </CardHeader>
      <CardContent className="px-2 sm:p-6">
        {error && (
          <p role="alert" className="mb-2 text-sm text-destructive">
            {error}
          </p>
        )}
        {keys.length === 0 ? (
          <p className="flex h-[220px] items-center justify-center text-sm text-muted-foreground">
            No inference spend in {monthLabel(view.year, view.month)}
          </p>
        ) : (
          <ChartContainer config={config} className="aspect-auto h-[220px] w-full">
            <LineChart accessibilityLayer data={data} margin={{ left: 12, right: 12 }}>
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="date"
                tickLine={false}
                axisLine={false}
                tickMargin={8}
                minTickGap={32}
                tickFormatter={(value: string) =>
                  new Date(value).toLocaleDateString("en-US", {
                    month: "short",
                    day: "numeric",
                    timeZone: "UTC",
                  })
                }
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                width={56}
                tickFormatter={(value: number) => `$${value.toFixed(value < 1 ? 2 : 0)}`}
              />
              <ChartTooltip
                content={
                  <ChartTooltipContent
                    labelFormatter={(value) =>
                      new Date(String(value)).toLocaleDateString("en-US", {
                        month: "short",
                        day: "numeric",
                        year: "numeric",
                        timeZone: "UTC",
                      })
                    }
                    formatter={(value, name, item) => (
                      <span className="flex w-full items-center gap-2">
                        <span
                          className="size-2.5 shrink-0 rounded-[2px]"
                          style={{ background: item.color }}
                        />
                        <span className="text-muted-foreground">
                          {config[String(name)]?.label ?? name}
                        </span>
                        <span className="ml-auto font-mono tabular-nums">
                          {dollars(Number(value) * 1_000_000)}
                        </span>
                      </span>
                    )}
                  />
                }
              />
              <ChartLegend content={<ChartLegendContent />} />
              {keys.map((id) => (
                <Line
                  key={id}
                  dataKey={id}
                  type="monotone"
                  stroke={`var(--color-${id})`}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ChartContainer>
        )}
      </CardContent>
    </Card>
  );
}
