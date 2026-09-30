import {
  AgentHealthStatus,
  type AgentHealthHour,
  type AgentMetrics,
} from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { Badge } from "@/components/ui/badge";

export function HealthBadge({ metrics }: { metrics?: AgentMetrics }) {
  const healthy = metrics?.health === AgentHealthStatus.HEALTHY;
  const unhealthy = metrics?.health === AgentHealthStatus.UNHEALTHY;
  const degraded = metrics?.health === AgentHealthStatus.DEGRADED;
  const label = healthy ? "Healthy" : degraded ? "Degraded" : unhealthy ? "Unhealthy" : "Unknown";
  return (
    <Badge
      variant="outline"
      className={
        healthy
          ? "border-emerald-600/20 bg-emerald-600/10 text-emerald-700 dark:text-emerald-400"
          : degraded
            ? "border-orange-500/20 bg-orange-500/10 text-orange-700 dark:text-orange-400"
            : unhealthy
              ? "border-destructive/20 bg-destructive/10 text-destructive"
              : "text-muted-foreground"
      }
      title={
        metrics?.lastHealthCheckAt
          ? `Last check: ${new Date(Number(metrics.lastHealthCheckAt.seconds) * 1000).toLocaleString()}. Checks older than 90 seconds are unknown.`
          : "No deployment health observation yet."
      }
    >
      {label}
    </Badge>
  );
}

/** Twelve hourly buckets, oldest first; shared by agents and remote tool servers. */
export function HealthHistory({ hours = [] }: { hours?: AgentHealthHour[] }) {
  const buckets = Array.from({ length: 12 }, (_, index) => {
    const hour = hours[index];
    const state = !hour?.totalChecks
      ? "unknown"
      : hour.failedChecks
        ? "unhealthy"
        : hour.degradedChecks
          ? "degraded"
          : "healthy";
    const time = hour?.hourStart
      ? new Date(Number(hour.hourStart.seconds) * 1000)
          .toISOString()
          .slice(0, 16)
          .replace("T", " ") + " UTC"
      : "No observations";
    const label = !hour?.totalChecks
      ? `${time}: no data`
      : `${time}: ${hour.failedChecks} failed, ${hour.degradedChecks ?? 0} degraded of ${hour.totalChecks} checks`;
    return { state, label };
  });
  const failed = buckets.filter((bucket) => bucket.state === "unhealthy").length;
  const partial = buckets.filter((bucket) => bucket.state === "degraded").length;
  const good = buckets.filter((bucket) => bucket.state === "healthy").length;
  return (
    <div
      role="img"
      aria-label={`Last 12 hours: ${good} healthy, ${partial} degraded, ${failed} unhealthy, ${12 - good - partial - failed} with no data`}
      className="flex w-fit items-center gap-1"
    >
      {buckets.map((bucket, index) => (
        <span
          key={index}
          data-health={bucket.state}
          title={bucket.label}
          className={`block h-6 w-2 rounded-xs ${bucket.state === "healthy" ? "bg-emerald-600 dark:bg-emerald-400" : bucket.state === "degraded" ? "bg-orange-500 dark:bg-orange-400" : bucket.state === "unhealthy" ? "bg-destructive" : "bg-muted-foreground/20"}`}
        />
      ))}
    </div>
  );
}

export function responseTime(milliseconds: number | undefined) {
  if (milliseconds === undefined) return "—";
  if (milliseconds < 1000) return `${Math.round(milliseconds)} ms`;
  return `${(milliseconds / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })} s`;
}
