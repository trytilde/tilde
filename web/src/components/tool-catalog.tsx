import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  PlusIcon,
  SearchIcon,
  ShieldCheckIcon,
  TagIcon,
  XIcon,
} from "lucide-react";
import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import type { ProviderTool, ToolHost } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { connections, tools } from "@/client";
import { cn } from "@/lib/utils";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { randomUUID } from "@/lib/browser-crypto";
import { AccountSetupDialog, credentialFields } from "./tool-account-setup";
import { ConnectionSetupDialog, type Brokering } from "./connection-setup-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetTitle,
} from "@/components/ui/sheet";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/*
 * The "configure a tool" catalog, reproduced from trytilde/dispatch (plugins-catalog.tsx) on
 * Tilde's model: a provider with tool-capable connection types is a catalog entry, each of its
 * tool connections is an "account", and an agent picks the tools it uses from an account on its
 * Tools tab. A remote server without a provider is an entry whose only account is itself.
 */

export type CatalogAccount = {
  id: string;
  name: string;
  status?: string;
  /** What the account serves tools from: a connection, or a remote server and its tools. */
  source: { connectionId: string } | { toolHostId: string; tools: ProviderTool[] };
};
export type CatalogProvider = {
  id: string;
  name: string;
  description: string;
  categories: string[];
  iconUrl?: string;
  /** Absent for a remote server without a provider: it cannot gain accounts. */
  provider?: Provider;
  accounts: CatalogAccount[];
  /** Set for a remote server's entry; its management page is linked from the dialog. */
  hostId?: string;
};

export function message(error: unknown) {
  return error instanceof Error ? error.message : "Request failed.";
}
export function statusLabel(status: string) {
  return (
    {
      ready: "Ready",
      requires_action: "Setup pending",
      failed: "Failed",
      disconnected: "Disconnected",
      available: "Available",
      offline: "Offline",
    }[status] ?? status
  );
}
export function toolMethods(provider: Provider) {
  return provider.connectionTypes.filter((type) => type.capabilities.includes(Capability.TOOL));
}
/** An MCP server added by URL: a registered provider whose every method is served over MCP. */
export function isMcpServer(provider: Provider) {
  return (
    provider.kind.case !== "builtIn" &&
    provider.connectionTypes.length > 0 &&
    provider.connectionTypes.every((type) => type.mcpServer)
  );
}
/** A provider ID derived from a name. */
export function defaultSlug(name: string) {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "_")
      .replace(/^[_-]+/, "")
      .slice(0, 32) || "tools"
  );
}

/** Disconnected connections have no credentials left; the catalog treats them as removed. */
export function connectionAccounts(providerId: string, list: Connection[]): CatalogAccount[] {
  return list
    .filter((c) => c.providerId === providerId && c.status !== "disconnected")
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((c) => ({
      id: c.id,
      name: c.name,
      status: c.status,
      source: { connectionId: c.id },
    }));
}
export function providerEntry(provider: Provider, list: Connection[]): CatalogProvider {
  return {
    id: provider.id,
    name: provider.name,
    description: provider.instructions ?? "",
    categories: provider.categories,
    iconUrl: provider.iconUrl,
    provider,
    accounts: connectionAccounts(provider.id, list),
  };
}
/** A server that publishes a provider is used through that provider's accounts. */
export function hostEntry(
  host: ToolHost,
  providers: Provider[],
  list: Connection[],
): CatalogProvider {
  const provider = host.providerId && providers.find((p) => p.id === host.providerId);
  if (provider) return { ...providerEntry(provider, list), hostId: host.id };
  return {
    id: `host:${host.id}`,
    name: host.name,
    description: "",
    categories: [],
    hostId: host.id,
    accounts: [
      {
        id: host.id,
        name: host.name,
        source: { toolHostId: host.id, tools: host.tools },
      },
    ],
  };
}

export function unique(values: readonly string[]) {
  return [...new Set(values)];
}
export function categoryLabel(value: string) {
  const text = value.replaceAll(/[_-]+/g, " ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}
export function compareCategories(left: string, right: string) {
  const leftIsOther = left.trim().toLowerCase() === "other";
  const rightIsOther = right.trim().toLowerCase() === "other";
  if (leftIsOther !== rightIsOther) return leftIsOther ? 1 : -1;
  return left.localeCompare(right);
}
function capabilityMark(name: string) {
  return name
    .split(/\s+/)
    .map((word) => word[0])
    .join("")
    .slice(0, 3)
    .toUpperCase();
}
function capabilityColor(id: string) {
  let hash = 0;
  for (const character of id) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 48% 43%)`;
}
function indefiniteArticle(value: string) {
  const word = value.trim().split(/\s/)[0] ?? "";
  // Acronyms are read letter by letter: "an MCP server", "an SMS provider".
  if (/^[A-Z]{2,}$/.test(word)) return /^[AEFHILMNORSX]/.test(word) ? "an" : "a";
  return /^[aeiou]/i.test(word) ? "an" : "a";
}
export function setupTitle(providerName: string) {
  return `Add ${indefiniteArticle(providerName)} ${providerName} account`;
}

/** 45px provider tile (28px `small`); letters on a colour hashed from the id when there is no
 * usable image. */
export function CatalogIcon({
  id,
  name,
  iconUrl,
  small = false,
}: {
  id: string;
  name: string;
  iconUrl?: string;
  small?: boolean;
}) {
  const [failed, setFailed] = useState<string>();
  const image = iconUrl && iconUrl !== failed ? iconUrl : undefined;
  return (
    <span
      aria-hidden="true"
      data-slot="provider-icon"
      className={cn(
        "grid shrink-0 place-items-center font-bold tracking-[-0.02em] text-white",
        small ? "size-7 rounded-md text-[9px]" : "size-[45px] rounded-[10px] text-[11px]",
        image && "bg-background dark:bg-white",
      )}
      style={image ? undefined : { backgroundColor: capabilityColor(id) }}
    >
      {image ? (
        <img
          alt=""
          className={cn(
            "h-auto w-auto object-contain",
            small ? "max-h-5 max-w-5" : "max-h-8 max-w-8",
          )}
          referrerPolicy="no-referrer"
          onError={() => setFailed(image)}
          src={image}
        />
      ) : (
        capabilityMark(name)
      )}
    </span>
  );
}

export function CatalogProviderRow({
  entry,
  onOpen,
}: {
  entry: CatalogProvider;
  onOpen: () => void;
}) {
  return (
    <li className="min-w-0">
      <button
        className="flex w-full min-w-0 cursor-pointer items-center gap-3 rounded-2xl bg-transparent px-3 py-[9.5px] text-left hover:bg-foreground/5 focus-visible:bg-foreground/5 focus-visible:outline-none"
        onClick={onOpen}
        type="button"
      >
        <CatalogIcon id={entry.id} name={entry.name} iconUrl={entry.iconUrl} />
        <div className="flex min-w-0 flex-1 flex-col gap-px">
          <h3 className="m-0 truncate text-[13px] leading-[18px] font-medium text-foreground">
            {entry.name}
          </h3>
          <p className="m-0 truncate text-[13px] leading-[18px] text-foreground/60">
            {entry.description}
          </p>
        </div>
      </button>
    </li>
  );
}

export function CatalogSection({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id}>
      <h3
        className="m-0 px-2 pt-2 pb-1.5 text-[13px] leading-[18px] font-medium text-foreground/40"
        id={id}
      >
        {title}
      </h3>
      <ul className="m-0 grid list-none grid-cols-2 gap-x-2 gap-y-0.5 p-0 max-[980px]:grid-cols-1">
        {children}
      </ul>
    </section>
  );
}

// Filter controls follow the trace filter bar: 26px square-edged triggers and menus in 11px mono.
const filterTrigger =
  "trace-controls flex h-[26px] shrink-0 cursor-pointer items-center gap-2 rounded-none border border-border bg-background px-2 text-[11px] shadow-none outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/40 data-popup-open:bg-muted";
const menuContent =
  "trace-controls rounded-none border border-border bg-popover p-0 shadow-lg ring-0";
const menuItem = "h-7 rounded-none border-b border-border/50 px-2 text-[11px] last:border-b-0";

export function CategoryFilter({
  categories,
  selectedCategory,
  onSelect,
}: {
  categories: readonly string[];
  selectedCategory: string | null;
  onSelect: (category: string | null) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={selectedCategory ? `Category: ${categoryLabel(selectedCategory)}` : "Category"}
        className={filterTrigger}
      >
        <TagIcon aria-hidden="true" className="size-3 text-muted-foreground" />
        <span className="max-w-32 truncate">
          {selectedCategory ? categoryLabel(selectedCategory) : "Category"}
        </span>
        <ChevronDownIcon aria-hidden="true" className="size-3 text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className={cn("min-w-[200px]", menuContent)}>
        <DropdownMenuItem className={menuItem} onClick={() => onSelect(null)}>
          All categories
        </DropdownMenuItem>
        <DropdownMenuSeparator className="-mx-0 my-0" />
        {categories.map((category) => (
          <DropdownMenuCheckboxItem
            checked={selectedCategory === category}
            className={cn(menuItem, "pl-8")}
            closeOnClick
            key={category}
            onCheckedChange={() => onSelect(selectedCategory === category ? null : category)}
          >
            {categoryLabel(category)}
          </DropdownMenuCheckboxItem>
        ))}
        {categories.length === 0 ? (
          <DropdownMenuItem className={menuItem} disabled>
            No categories available
          </DropdownMenuItem>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function CatalogSearchField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (query: string) => void;
}) {
  return (
    <label className="trace-controls relative flex h-[26px] min-w-40 flex-1 items-center rounded-md border border-input bg-background focus-within:ring-2 focus-within:ring-ring/40 sm:max-w-sm">
      <SearchIcon
        aria-hidden="true"
        className="pointer-events-none absolute left-1.5 size-3 text-muted-foreground"
      />
      <span className="sr-only">{label}</span>
      <input
        className="h-full w-full min-w-0 bg-transparent pr-2 pl-6 text-[11px] text-foreground outline-none placeholder:text-muted-foreground"
        onChange={(event) => onChange(event.target.value)}
        placeholder={label}
        type="search"
        value={value}
      />
    </label>
  );
}

export function CatalogSkeleton() {
  return (
    <div
      aria-label="Loading tools"
      className="grid grid-cols-2 gap-x-2 gap-y-0.5 max-[980px]:grid-cols-1"
      role="status"
    >
      {Array.from({ length: 6 }, (_, index) => (
        <div
          aria-hidden="true"
          className="flex h-16 min-w-0 animate-pulse items-center gap-3 rounded-2xl px-3 py-[9.5px] motion-reduce:animate-none"
          key={index}
        >
          <span className="size-[45px] shrink-0 rounded-[10px] bg-foreground/10" />
          <span className="min-w-0 flex-1 space-y-2">
            <span
              className={cn(
                "block h-2.5 rounded-full bg-foreground/10",
                index % 3 === 0 ? "w-28" : index % 3 === 1 ? "w-36" : "w-24",
              )}
            />
            <span className="block h-2 w-[min(90%,240px)] rounded-full bg-foreground/5" />
          </span>
          <span className="flex -space-x-1.5 pl-2">
            <span className="size-[30px] rounded-full border-2 border-background bg-foreground/10" />
            <span className="size-[30px] rounded-full border-2 border-background bg-muted" />
          </span>
        </div>
      ))}
    </div>
  );
}

// Dialog chrome shared by the catalog dialogs: dispatch's radius, padding and type scale.
export const dialogChrome = "gap-0 rounded-2xl p-[22px]";
export const dialogTitle = "m-0 text-base leading-snug font-semibold text-foreground";
export const dialogButton = "h-8 gap-2 rounded-lg px-3.5 text-[13px]";

type Stage =
  | { kind: "detail" }
  | { kind: "setup"; typeId: string }
  | { kind: "frame"; setup: Brokering | null; error: string; connectionId: string };

/**
 * The provider detail panel and the account setup it opens. A connected account continues on
 * its own page, where it is given to agents; existing accounts are managed from Connections.
 * Mount it keyed by the entry; `onClose` fires when the last dialog closes.
 */
export function ToolProviderDialogs({
  entry,
  refresh,
  onClose,
  start = "detail",
}: {
  entry: CatalogProvider;
  refresh: () => Promise<void>;
  onClose: () => void;
  /** "setup" skips the detail panel and starts adding an account. */
  start?: "detail" | "setup";
}) {
  const navigate = useNavigate();
  const [stage, setStage] = useState<Stage>({ kind: "detail" });
  let canAddAccount = true;
  const [error, setError] = useState("");
  const provider = entry.provider;
  const methods = provider ? toolMethods(provider) : [];

  function reload() {
    void refresh().catch((e) => setError(message(e)));
  }
  function addAccount(typeId: string) {
    if (!provider) return;
    setError("");
    const chosen = methods.find((method) => method.id === typeId) ?? methods[0];
    // Provider-owned setup UI stays in its sandboxed page.
    if (chosen && !credentialFields(chosen)) {
      const id = randomUUID();
      setStage({
        kind: "frame",
        setup: { title: setupTitle(provider.name), url: "" },
        error: "",
        connectionId: id,
      });
      void connections
        .startConnection({
          id,
          name: provider.name,
          providerId: provider.id,
          typeId: chosen.id,
          assignments: [],
        })
        .then((result) => {
          reload();
          setStage((current) =>
            current.kind === "frame"
              ? {
                  ...current,
                  connectionId: result.connection?.id ?? id,
                  setup: {
                    title: setupTitle(provider.name),
                    url: new URL(result.brokeringUrl, window.location.origin).href,
                  },
                }
              : current,
          );
        })
        .catch((e) =>
          setStage((current) =>
            current.kind === "frame" ? { ...current, error: message(e) } : current,
          ),
        );
      return;
    }
    setStage({ kind: "setup", typeId: chosen?.id ?? "" });
  }
  // Before paint, so the detail panel never flashes.
  useLayoutEffect(() => {
    if (start === "setup") addAccount(methods[0]?.id ?? "");
    // Once, on mount: the dialogs are keyed by the entry.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  function connected(connectionId: string) {
    void navigate({
      to: "/tools/$toolId",
      params: { toolId: connectionId },
      search: { kind: "connection" },
    });
  }
  return (
    <>
      <Sheet
        open={stage.kind === "detail"}
        onOpenChange={(next) => {
          if (!next) onClose();
        }}
      >
        <SheetContent
          side="right"
          showCloseButton={false}
          className="w-full gap-0 overflow-y-auto p-0 data-[side=right]:w-full data-[side=right]:sm:w-1/2 data-[side=right]:sm:min-w-[560px] data-[side=right]:sm:max-w-none"
        >
          <SheetClose
            aria-label="Close"
            className="absolute top-4 right-4 grid size-7 cursor-pointer place-items-center rounded-md bg-transparent text-foreground/40 hover:bg-foreground/5 hover:text-foreground focus-visible:bg-foreground/5 focus-visible:text-foreground focus-visible:outline-none"
          >
            <XIcon className="size-3.5" />
          </SheetClose>
          <div className="flex flex-col gap-6 p-6">
            <header className="grid gap-1.5 pr-10">
              {entry.categories[0] ? (
                <p className="m-0 font-mono text-[11px] text-muted-foreground uppercase">
                  {entry.categories.map(categoryLabel).join(" · ")}
                </p>
              ) : null}
              <div className="flex items-center gap-2.5">
                <CatalogIcon id={entry.id} name={entry.name} iconUrl={entry.iconUrl} small />
                <SheetTitle className="m-0 text-lg font-semibold">{entry.name}</SheetTitle>
              </div>
              <SheetDescription className="m-0 text-[13px] leading-5 text-foreground/60">
                {entry.description || "Manage which agents can use this capability."}
              </SheetDescription>
              {entry.hostId ? (
                <Link
                  to="/tools/$toolId"
                  params={{ toolId: entry.hostId }}
                  search={{ kind: "host" }}
                  className="w-fit text-xs font-medium text-foreground/60 hover:text-foreground hover:underline"
                >
                  Manage server
                </Link>
              ) : null}
            </header>

            {error ? (
              <p
                className="m-0 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
                role="alert"
              >
                {error}
              </p>
            ) : null}

            {methods.length || (provider && canAddAccount) ? (
              <div className="-mb-3 flex flex-wrap items-center justify-between gap-2">
                <div className="flex flex-wrap items-center gap-2">
                  {methods.length ? (
                    <>
                      <p className="m-0 font-mono text-[10px] text-muted-foreground uppercase">
                        Supported auth
                      </p>
                      {methods.map((method) => (
                        <Badge key={method.id} variant="outline" className="gap-1 text-[10px]">
                          <ShieldCheckIcon className="size-3" />
                          {method.name}
                        </Badge>
                      ))}
                    </>
                  ) : null}
                </div>
                {provider && canAddAccount ? (
                  <AddAccountMenu methods={methods} onSelect={(typeId) => addAccount(typeId)} />
                ) : null}
              </div>
            ) : null}

            <AvailableTools entry={entry} />
          </div>
        </SheetContent>
      </Sheet>

      {stage.kind === "setup" && provider ? (
        <AccountSetupDialog
          provider={provider}
          initialTypeId={stage.typeId}
          iconId={entry.id}
          onStarted={reload}
          onComplete={connected}
          onCustom={(setup, connectionId) =>
            setStage({ kind: "frame", setup, error: "", connectionId })
          }
          onClose={() => {
            reload();
            onClose();
          }}
        />
      ) : null}

      {stage.kind === "frame" ? (
        <FrameSetup
          stage={stage}
          onComplete={connected}
          onClose={() => {
            reload();
            onClose();
          }}
        />
      ) : null}
    </>
  );
}

/** "Add account", choosing the auth method first when the provider has several. */
function AddAccountMenu({
  methods,
  onSelect,
}: {
  methods: Provider["connectionTypes"];
  onSelect: (typeId: string) => void;
}) {
  if (methods.length <= 1)
    return (
      <Button className="shrink-0 gap-1.5" onClick={() => onSelect(methods[0]?.id ?? "")}>
        <PlusIcon className="size-4" />
        Add account
      </Button>
    );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button className="shrink-0 gap-1.5" />}>
        <PlusIcon className="size-4" />
        Add account
        <ChevronDownIcon className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        <p className="m-0 px-2.5 py-1.5 text-xs text-muted-foreground">Choose auth method</p>
        <DropdownMenuSeparator />
        {methods.map((method) => (
          <DropdownMenuItem key={method.id} onClick={() => onSelect(method.id)}>
            <ShieldCheckIcon className="size-3.5" />
            {method.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/**
 * Every tool the provider offers, as trytilde/api's provider page lists them: built-in tools,
 * a remote server's tools, or an MCP server's advertised list before any connection.
 */
function AvailableTools({ entry }: { entry: CatalogProvider }) {
  const hosted = entry.accounts.find((account) => "toolHostId" in account.source);
  const of =
    !entry.provider && hosted && "toolHostId" in hosted.source
      ? { toolHostId: hosted.source.toolHostId }
      : { providerId: entry.id };
  return (
    <ToolTable
      of={of}
      // Nothing to list yet: an added MCP server before its first account, or a remote server
      // that has published no tools.
      empty={
        <p className="m-0 rounded-lg border px-3 py-4 text-xs leading-5 text-muted-foreground">
          {entry.provider && isMcpServer(entry.provider)
            ? "The server's tools are discovered when its first account connects."
            : "No tools have been published yet."}
        </p>
      }
    />
  );
}

/** Whose tools a ToolTable lists: a connection's, a tool host's or a catalog provider's. */
export type ToolsOf = { connectionId: string } | { toolHostId: string } | { providerId: string };
/**
 * A read-only table of tools the server searches as the user types. `empty` replaces it when
 * there are no tools at all; a change of `version` reloads them.
 */
export function ToolTable({
  of,
  empty,
  version = 0,
}: {
  of: ToolsOf;
  empty: ReactNode;
  version?: number;
}) {
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const [shown, setShown] = useState<ProviderTool[] | null>(null);
  const [error, setError] = useState("");
  const target = JSON.stringify(of);
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    void tools
      .listProviderTools(
        { connectionId: "", ...(JSON.parse(target) as ToolsOf), search: search || undefined },
        { signal: abort.signal },
      )
      .then((response) => setShown(response.tools))
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      });
    return () => abort.abort();
  }, [target, search, version]);
  if (!search && shown?.length === 0 && !error) return empty;
  return (
    <section aria-label="Available tools" className="overflow-hidden rounded-lg border">
      <div className="flex flex-wrap items-center justify-between gap-3 bg-muted/60 px-3 py-2">
        <div>
          <h3 className="m-0 text-[13px] font-medium">Available tools</h3>
          <p className="m-0 font-mono text-[11px] text-muted-foreground uppercase">
            <span className="tabular-nums">{shown?.length ?? 0}</span> shown
          </p>
        </div>
        <CatalogSearchField label="Search tools" value={query} onChange={setQuery} />
      </div>
      {error ? (
        <p className="m-0 px-3 py-4 text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : shown === null ? (
        <p className="m-0 px-3 py-4 text-xs text-muted-foreground">Loading tools…</p>
      ) : shown.length === 0 ? (
        <p className="m-0 px-3 py-6 text-center text-xs text-muted-foreground">
          No tools match this search.
        </p>
      ) : (
        <table className="w-full table-fixed text-left">
          <thead className="bg-muted/50">
            <tr className="border-y text-[11px] text-muted-foreground">
              <th className="w-[34%] px-3 py-2 font-medium">Tool</th>
              <th className="px-3 py-2 font-medium">Description</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((tool) => (
              <tr key={tool.name} className="border-b last:border-b-0 align-top">
                <td className="px-3 py-2 font-mono text-xs font-medium break-words">{tool.name}</td>
                <td className="px-3 py-2 text-xs text-muted-foreground">
                  <span className="line-clamp-2">{tool.description}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

/** The hosted setup page of a provider-owned (custom) method; completion opens the new account. */
function FrameSetup({
  stage,
  onComplete,
  onClose,
}: {
  stage: Extract<Stage, { kind: "frame" }>;
  onComplete: (connectionId: string) => void;
  onClose: () => void;
}) {
  const [done, setDone] = useState(false);
  useWindowMessage((event) => {
    // The hosted page announces completion to its parent; it carries no credentials.
    if (
      !done &&
      event.origin === window.location.origin &&
      event.data?.type === "tilde.connection.complete" &&
      event.data.connectionId === stage.connectionId
    ) {
      setDone(true);
      onComplete(stage.connectionId);
    }
  });
  return <ConnectionSetupDialog setup={stage.setup} error={stage.error} onClose={onClose} />;
}

function useWindowMessage(handler: (event: MessageEvent) => void) {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    const listener = (event: MessageEvent) => latest.current(event);
    window.addEventListener("message", listener);
    return () => window.removeEventListener("message", listener);
  }, []);
}
