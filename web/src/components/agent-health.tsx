import { AgentHealthStatus, type AgentMetrics } from "@/gen/tilde/types/v1/agent_pb.js";
import { Badge } from "@/components/ui/badge";

export function HealthBadge({ metrics }: { metrics?: AgentMetrics }) {
  const healthy = metrics?.health === AgentHealthStatus.HEALTHY;
  const unhealthy = metrics?.health === AgentHealthStatus.UNHEALTHY;
  const label = healthy ? "Healthy" : unhealthy ? "Unhealthy" : "Unknown";
  return (
    <Badge
      variant="outline"
      className={
        healthy
          ? "border-emerald-600/20 bg-emerald-500/10 text-emerald-700"
          : unhealthy
            ? "border-red-600/20 bg-red-500/10 text-red-700"
            : "text-muted-foreground"
      }
      title={
        metrics?.lastHealthCheckAt
          ? `Last check: ${new Date(Number(metrics.lastHealthCheckAt.seconds) * 1000).toLocaleString()}. Checks older than 90 seconds are unknown.`
          : "No health observation for this endpoint yet."
      }
    >
      {label}
    </Badge>
  );
}

export function HealthHistory({ metrics }: { metrics?: AgentMetrics }) {
  const hours = metrics?.healthHistory ?? [];
  const buckets = Array.from({ length: 12 }, (_, index) => {
    const hour = hours[index];
    const state = !hour?.totalChecks ? "unknown" : hour.failedChecks ? "unhealthy" : "healthy";
    const time = hour?.hourStart
      ? new Date(Number(hour.hourStart.seconds) * 1000)
          .toISOString()
          .slice(0, 16)
          .replace("T", " ") + " UTC"
      : "No observations";
    const label = !hour?.totalChecks
      ? `${time}: no data`
      : `${time}: ${hour.failedChecks} failed of ${hour.totalChecks} checks`;
    return { state, label };
  });
  const failed = buckets.filter((bucket) => bucket.state === "unhealthy").length;
  const good = buckets.filter((bucket) => bucket.state === "healthy").length;
  return (
    <div
      role="img"
      aria-label={`Last 12 hours: ${good} healthy, ${failed} unhealthy, ${12 - good - failed} with no data`}
      className="flex w-fit items-center gap-1"
    >
      {buckets.map((bucket, index) => (
        <span
          key={index}
          data-health={bucket.state}
          title={bucket.label}
          className={`block h-6 w-2 rounded-xs ${bucket.state === "healthy" ? "bg-emerald-500" : bucket.state === "unhealthy" ? "bg-red-500" : "bg-muted-foreground/20"}`}
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
