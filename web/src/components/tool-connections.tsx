import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate, useParams } from "@tanstack/react-router";
import { CopyIcon } from "lucide-react";
import { Capability, type Provider } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ToolHostType } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import {
  ProviderSource,
  type ListConnectionsRequest,
  type ListProvidersRequest,
} from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import type { Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, toolHosts } from "@/client";
import {
  CatalogProviderRow,
  CatalogSearchField,
  CatalogSection,
  CatalogSkeleton,
  CategoryFilter,
  ToolProviderDialogs,
  compareCategories,
  categoryLabel,
  message,
  providerEntry,
  type CatalogProvider,
} from "./tool-catalog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export { isMcpServer, message, statusLabel, toolMethods } from "./tool-catalog";
type ProviderQuery = Pick<ListProvidersRequest, "search" | "category" | "source">;
type ConnectionQuery = Pick<ListConnectionsRequest, "search" | "providerId" | "status" | "source">;
/**
 * Every tool-capable provider the server's filters keep, and the categories the filters'
 * source offers (for a category menu, whatever the search and category).
 */
export async function loadToolProviders(query: ProviderQuery = {}, signal?: AbortSignal) {
  const providers: Provider[] = [];
  let categories: string[] = [];
  let pageToken = "";
  do {
    const page = await connections.listProviders(
      { ...query, capability: Capability.TOOL, pageSize: 100, pageToken },
      { signal },
    );
    providers.push(...page.providers);
    categories = page.categories;
    pageToken = page.nextPageToken;
  } while (pageToken);
  return { providers, categories };
}
export async function loadToolConnections(query: ConnectionQuery = {}, signal?: AbortSignal) {
  const all: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections(
      { ...query, pageSize: 100, pageToken, capability: Capability.TOOL },
      { signal },
    );
    all.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return all;
}

/**
 * The tool catalog (dispatch's "configure a tool" page): providers grouped by their first
 * category. The server filters them by category and search. Dispatch's All/Personal/Bots scope
 * toggle is omitted: management users have no personal accounts. Providers published by remote
 * servers and MCP servers added by URL are opened from the Remote servers page instead.
 */
export function ToolsPage() {
  const [entries, setEntries] = useState<CatalogProvider[]>([]);
  const [categories, setCategories] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const navigate = useNavigate();
  // The open provider's panel is a path of its own: /tools/catalog/$providerId.
  const { providerId: openId } = useParams({ strict: false });
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [catalog, list] = await Promise.all([
        loadToolProviders(
          {
            source: ProviderSource.CATALOG,
            search: search || undefined,
            category: selectedCategory ?? undefined,
          },
          signal,
        ),
        loadToolConnections({}, signal),
      ]);
      setEntries(catalog.providers.map((provider) => providerEntry(provider, list)));
      setCategories([...catalog.categories].sort(compareCategories));
      setError("");
    },
    [search, selectedCategory],
  );
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((error) => {
        if (!abort.signal.aborted) setError(message(error));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);

  const groups = new Map<string, CatalogProvider[]>();
  for (const entry of entries) {
    const category = selectedCategory ?? entry.categories[0] ?? "Other";
    groups.set(category, [...(groups.get(category) ?? []), entry]);
  }
  const open = entries.find((entry) => entry.id === openId);

  return (
    <section aria-label="Tools" className="flex min-h-full min-w-0 flex-col text-foreground">
      <div className="shrink-0 border-b bg-background">
        <div
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
          role="group"
          aria-label="Catalog filter bar"
        >
          <CategoryFilter
            categories={categories}
            selectedCategory={selectedCategory}
            onSelect={setSelectedCategory}
          />
          <CatalogSearchField label="Search tools" value={query} onChange={setQuery} />
        </div>
      </div>

      <section aria-label="Tool providers" className="p-4 lg:p-6">
        {error ? (
          <div
            className="mx-2 mb-2 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
            role="alert"
          >
            {error}
          </div>
        ) : null}
        {loading ? (
          <CatalogSkeleton />
        ) : (
          <div className="space-y-3">
            {[...groups]
              .sort(([left], [right]) => compareCategories(left, right))
              .map(([category, items]) => (
                <CatalogSection key={category} title={categoryLabel(category)}>
                  {items.map((entry) => (
                    <CatalogProviderRow
                      entry={entry}
                      key={entry.id}
                      onOpen={() =>
                        void navigate({
                          to: "/tools/catalog/$providerId",
                          params: { providerId: entry.id },
                        })
                      }
                    />
                  ))}
                </CatalogSection>
              ))}
          </div>
        )}
        {!loading && entries.length === 0 ? (
          <div className="mt-2.5 rounded-xl border border-dashed border-foreground/15 p-9 text-center text-[12.5px] text-foreground/40">
            No tools match these filters.
          </div>
        ) : null}
      </section>

      {open ? (
        <ToolProviderDialogs
          entry={open}
          key={open.id}
          refresh={() => refresh()}
          onClose={() => void navigate({ to: "/tools/catalog" })}
        />
      ) : null}
    </section>
  );
}

type HostForm = { name: string; functionArn: string };
/** How a Tilde tool server is deployed: it dials out with a token, or Tilde invokes a Lambda. */
export type Deployment = "connected" | "lambda";
export const DEPLOYMENTS: Record<Deployment, string> = {
  connected: "Self-hosted (connects out)",
  lambda: "AWS Lambda",
};
/** Registers a tool host. A connected host's token is shown once; only its digest is stored. */
export function RegisterToolHostDialog({
  open,
  deployment,
  onOpenChange,
  onRegistered,
}: {
  open: boolean;
  /** Chosen before the dialog opens. */
  deployment: Deployment;
  onOpenChange: (open: boolean) => void;
  onRegistered: () => void;
}) {
  const form = useForm<HostForm>({
    defaultValues: { name: "", functionArn: "" },
  });
  const [token, setToken] = useState("");
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lambda = deployment === "lambda";
  function close() {
    onOpenChange(false);
    setToken("");
    setCopied(false);
    setError("");
    form.reset();
  }
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add Tilde tool server · {DEPLOYMENTS[deployment]}</DialogTitle>
          <DialogDescription>
            {lambda
              ? "Your own tool backend as an AWS Lambda function, invoked by its ARN with Tilde's AWS credentials."
              : "Your own tool backend, running anywhere. It connects out to Tilde with the token shown next, so it needs no inbound access."}
          </DialogDescription>
        </DialogHeader>
        {token ? (
          <div className="space-y-3">
            <Label htmlFor="tool-host-token">Tool host token</Label>
            <div className="flex gap-2">
              <Input id="tool-host-token" readOnly value={token} aria-label="Tool host token" />
              <Button
                variant="outline"
                onClick={() =>
                  void navigator.clipboard.writeText(token).then(() => setCopied(true))
                }
              >
                <CopyIcon />
                {copied ? "Copied" : "Copy"}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Shown once. Set it as TILDE_TOOL_HOST_TOKEN for the host process.
            </p>
            <DialogFooter>
              <Button onClick={close}>Done</Button>
            </DialogFooter>
          </div>
        ) : (
          <form
            className="space-y-4"
            onSubmit={form.handleSubmit(async (values) => {
              setBusy(true);
              setError("");
              try {
                const result = await toolHosts.registerToolHost({
                  name: values.name.trim(),
                  type: lambda ? ToolHostType.LAMBDA : ToolHostType.CONNECTED,
                  functionArn: lambda ? values.functionArn.trim() : undefined,
                });
                onRegistered();
                if (result.token) setToken(result.token);
                else close();
              } catch (e) {
                setError(message(e));
              } finally {
                setBusy(false);
              }
            })}
          >
            <div className="space-y-2">
              <Label htmlFor="tool-host-name">Name</Label>
              <Input id="tool-host-name" {...form.register("name", { required: true })} />
            </div>
            {lambda && (
              <div className="space-y-2">
                <Label htmlFor="tool-host-arn">Function ARN</Label>
                <Input
                  id="tool-host-arn"
                  placeholder="arn:aws:lambda:region:account:function:name"
                  {...form.register("functionArn", { required: lambda })}
                />
              </div>
            )}
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
            <DialogFooter>
              <Button type="button" variant="outline" onClick={close} disabled={busy}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                Add server
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
