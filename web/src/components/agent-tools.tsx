import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { timestampDate } from "@bufbuild/protobuf/wkt";
import {
  CableIcon,
  CheckIcon,
  ChevronRightIcon,
  CodeIcon,
  InfoIcon,
  PencilIcon,
  PlusIcon,
  Trash2Icon,
  WrenchIcon,
  XIcon,
  ZapIcon,
} from "lucide-react";
import { ToolDisplay } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import type { Connection, Provider } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import {
  ToolMode,
  type AgentTool,
  type ProviderTool,
  type ToolHost,
  type ToolSource,
} from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { toolHosts, tools } from "@/client";
import { ProviderIcon } from "./provider-icon";
import { servingContents } from "./agent-deployment";
import { ChooseDialog } from "./choose-dialog";
import { loadToolConnections, loadToolProviders, message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/** A connection or provider-less tool host an agent can use tools from. */
type Origin = { name: string; detail: string; iconUrl?: string } & (
  | { connectionId: string }
  | { toolHostId: string; tools: ProviderTool[] }
);
/** One of the agent's sources, with every tool its origin offers. */
type Entry = { source: ToolSource; name: string; iconUrl?: string; offered: ProviderTool[] };
/** A tool a source offers, as far as the table needs it. */
type Offered = Pick<ProviderTool, "name" | "description" | "summary">;

const key = (origin: Origin) =>
  "connectionId" in origin ? origin.connectionId : origin.toolHostId;
function connectionOrigin(connection: Connection, providers: Map<string, Provider>): Origin {
  const provider = providers.get(connection.providerId);
  return {
    name: connection.name,
    detail: provider?.name ?? connection.providerId,
    iconUrl: provider?.iconUrl,
    connectionId: connection.id,
  };
}
// Only hosts without a provider are origins: the others are used through their provider's
// connections.
function hostOrigin(host: ToolHost): Origin {
  return { name: host.name, detail: "Tool server", toolHostId: host.id, tools: host.tools };
}

async function offeredTools(origin: Origin, signal?: AbortSignal) {
  if ("toolHostId" in origin) return origin.tools;
  return (await tools.listProviderTools({ connectionId: origin.connectionId }, { signal })).tools;
}

/**
 * The tools one agent uses, grouped by source (a tool connection or a tool host without a
 * provider): which offered tools it uses and which run in the background, and one setting for
 * whether all of them are listed or found by search. The agent's bundled tools, set in
 * its code, are shown read-only beside them (`bundledGroups`).
 */
export function AgentTools({ agentId }: { agentId: string }) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [providers, setProviders] = useState<Map<string, Provider>>(new Map());
  const [mode, setMode] = useState(ToolMode.DIRECT);
  const [bundled, setBundled] = useState<BundledGroup[]>([]);
  const [choosing, setChoosing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [owned, list, hosts, providers, offering, fromCode] = await Promise.all([
        tools.listToolSources({ agentId }, { signal }),
        loadToolConnections({}, signal),
        toolHosts.listToolHosts({ withProvider: false }, { signal }),
        loadToolProviders({}, signal),
        tools.getToolMode({ agentId }, { signal }),
        bundledGroups(agentId, signal),
      ]);
      setBundled(fromCode);
      setMode(offering.mode === ToolMode.DYNAMIC ? ToolMode.DYNAMIC : ToolMode.DIRECT);
      const byProvider = new Map<string, Provider>(providers.providers.map((p) => [p.id, p]));
      setProviders(byProvider);
      const origins = [
        ...list.map((c) => connectionOrigin(c, byProvider)),
        ...hosts.toolHosts.map(hostOrigin),
      ];
      const originOf = (source: ToolSource) =>
        origins.find((o) =>
          "connectionId" in o
            ? o.connectionId === source.connectionId
            : o.toolHostId === source.toolHostId,
        );
      const next = await Promise.all(
        owned.sources.map(async (source): Promise<Entry> => {
          const origin = originOf(source);
          return {
            source,
            name: origin?.name ?? source.slug,
            iconUrl: origin?.iconUrl,
            // A connection that is not ready has no readable tools; its chosen ones still show.
            offered: origin ? await offeredTools(origin, signal).catch(() => []) : [],
          };
        }),
      );
      setEntries(next.sort((a, b) => a.name.localeCompare(b.name)));
    },
    [agentId],
  );
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  async function act(work: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await work();
      await refresh();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-y-auto" aria-label="Tools">
      <div className="shrink-0 space-y-5 border-b px-4 py-5 lg:px-6">
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="w-1/2 min-w-72 space-y-3">
          <div className="min-w-0">
            <h3 className="text-sm font-medium">Tool discovery</h3>
            <p
              id="tool-mode-description"
              className="mt-1 text-xs leading-relaxed whitespace-normal text-muted-foreground"
            >
              {mode === ToolMode.DYNAMIC
                ? "The agent finds these tools with tools.search and runs them through tools.execute."
                : "Every tool switched on below is in the agent's tool list."}{" "}
              Applies to all of the agent's tools from its next run.
            </p>
          </div>
          <Tabs
            value={mode === ToolMode.DYNAMIC ? "dynamic" : "direct"}
            onValueChange={(value) => {
              const next = value === "dynamic" ? ToolMode.DYNAMIC : ToolMode.DIRECT;
              if (next !== mode) void act(() => tools.setToolMode({ agentId, mode: next }));
            }}
          >
            <TabsList
              aria-label="How the agent's tools are offered"
              aria-describedby="tool-mode-description"
            >
              <TabsTrigger value="direct" disabled={busy}>
                Listed directly
              </TabsTrigger>
              <TabsTrigger value="dynamic" disabled={busy}>
                Found by search
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </div>
      <div className="space-y-5 px-4 py-5 lg:px-6">
        <div className="flex justify-end">
          <DropdownMenu>
            <DropdownMenuTrigger
              disabled={busy}
              render={<Button className="cursor-pointer gap-2" />}
            >
              <PlusIcon />
              Add tool
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-56">
              <DropdownMenuItem onClick={() => setChoosing(true)}>
                <CableIcon />
                Choose existing
              </DropdownMenuItem>
              <DropdownMenuItem render={<Link to="/tools/connections" />}>
                <WrenchIcon />
                Connect a new tool
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <ChooseExisting
          open={choosing}
          providers={providers}
          used={entries.map(({ source }) => source.connectionId ?? source.toolHostId ?? "")}
          busy={busy}
          onClose={() => setChoosing(false)}
          // A new source starts with every tool off; the agent gets only what is switched on.
          onChoose={(origin) =>
            act(() =>
              tools.addToolSource({
                agentId,
                ...("connectionId" in origin
                  ? { connectionId: origin.connectionId }
                  : { toolHostId: origin.toolHostId }),
                toolNames: [],
              }),
            ).then(() => setChoosing(false))
          }
        />
        {entries.length || bundled.length ? (
          <div className="overflow-hidden rounded-xl border">
            <Table aria-label="Agent tools" className="table-fixed">
              <TableHeader className="bg-muted/50">
                <TableRow>
                  <TableHead className="h-11 w-16 px-5">
                    <span className="sr-only">Use</span>
                  </TableHead>
                  <TableHead className="h-11 w-64">Tool</TableHead>
                  <TableHead className="h-11">Description</TableHead>
                  <TableHead className="h-11 w-64">
                    <span className="flex items-center gap-1">
                      Displayed
                      <Tooltip>
                        <TooltipTrigger
                          render={
                            <button
                              type="button"
                              aria-label="About displayed"
                              className="inline-flex cursor-help text-muted-foreground [&_svg]:size-3.5"
                            />
                          }
                        >
                          <InfoIcon />
                        </TooltipTrigger>
                        <TooltipContent className="max-w-72">
                          How this tool's calls appear to people chatting with the agent. Full shows
                          the input and output, Summary only the summary and whether it finished,
                          Hidden nothing. Traces always keep full detail.
                        </TooltipContent>
                      </Tooltip>
                    </span>
                  </TableHead>
                  <TableHead className="h-11 w-28">Background</TableHead>
                  <TableHead className="h-11 w-24 px-5 text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((entry) => (
                  <SourceRows key={entry.source.id} entry={entry} busy={busy} act={act} />
                ))}
                {bundled.map((group) => (
                  <BundledRows key={group.id} group={group} />
                ))}
              </TableBody>
            </Table>
          </div>
        ) : (
          <div className="flex min-h-48 w-full items-center justify-center rounded-xl border border-dashed bg-card p-6 text-center text-sm text-muted-foreground">
            {loading ? "Loading tools…" : "No tools assigned to this agent."}
          </div>
        )}
      </div>
    </section>
  );
}

const DISPLAY = {
  full: ToolDisplay.FULL,
  summary: ToolDisplay.SUMMARY,
  hidden: ToolDisplay.HIDDEN,
} as const;
const displayKey = (display: ToolDisplay) =>
  display === ToolDisplay.SUMMARY ? "summary" : display === ToolDisplay.HIDDEN ? "hidden" : "full";
/** Every setting of one of the agent's tools; SetAgentTool replaces them all at once. */
function settings(tool: AgentTool) {
  return {
    isAsync: tool.isAsync,
    summary: tool.summary,
    description: tool.description,
    display: tool.display,
  };
}

const DISPLAY_TEXT = { full: "Full", summary: "Summary", hidden: "Hidden" } as const;

type BundledGroup = {
  id: string;
  /** Names the deployment when several serve the agent. */
  name: string;
  detail: string;
  tools: { name: string; summary: string; description: string; display: ToolDisplay }[];
};
/**
 * The agent's bundled tools, one group per deployment that can receive its invocations (as the
 * Skills tab groups bundled skills), with the tools `tilde deploy` declared for it. When none of
 * them declared any, the tools the agent's latest run registered stand in.
 */
async function bundledGroups(agentId: string, signal?: AbortSignal): Promise<BundledGroup[]> {
  const serving = await servingContents(agentId, signal);
  if (serving.some(({ contents }) => contents.tools.length))
    return serving.map(({ deployment, name, detail, contents }) => ({
      id: deployment.id,
      name: serving.length > 1 ? name : "",
      detail: `Declared by ${[name, detail].filter(Boolean).join(" · ")}`,
      tools: contents.tools,
    }));
  const registered = await tools.listBundledTools({ agentId }, { signal });
  return [
    {
      id: "registered",
      name: "",
      detail: registered.registeredAt
        ? `Registered by the run at ${timestampDate(registered.registeredAt).toLocaleString()}`
        : "None declared or registered yet. Export the agent's tools with defineTools / define_tools from its entry and tilde deploy declares them with the deployment.",
      tools: registered.tools,
    },
  ];
}

/**
 * Bundled tools: shipped in the agent's code and run in its process. They are configured in
 * code, so the group is read-only.
 */
function BundledRows({ group }: { group: BundledGroup }) {
  const count = group.tools.length;
  const [expanded, setExpanded] = useState(true);
  const name = group.name ? `From code · ${group.name}` : "From code";
  return (
    <>
      <TableRow className="bg-muted/40 hover:bg-muted/40">
        <TableCell className="h-14 px-5 py-2">
          <CodeIcon aria-hidden="true" className="size-5 text-muted-foreground" />
        </TableCell>
        <TableCell colSpan={5}>
          <span className="flex flex-wrap items-center gap-3">
            <GroupToggle name={name} expanded={expanded} onToggle={setExpanded} />
            <span className="font-medium">{name}</span>
            <Badge
              variant="secondary"
              title="Ships with the agent's code and runs in its process. Configure it in code, not here."
            >
              Bundled
            </Badge>
            <Badge variant="outline">
              {count} tool{count === 1 ? "" : "s"}
            </Badge>
            <span className="text-xs text-muted-foreground">{group.detail}</span>
          </span>
        </TableCell>
      </TableRow>
      {expanded &&
        group.tools.map((tool) => (
          <TableRow key={tool.name}>
            <TableCell />
            <TableCell className="whitespace-normal">
              <code className="text-xs font-medium">{tool.name}</code>
              <span className="block text-xs text-muted-foreground">
                {tool.summary || "(no summary)"}
              </span>
            </TableCell>
            <TableCell className="whitespace-normal">
              <span className="block text-xs text-muted-foreground">{tool.description}</span>
            </TableCell>
            <TableCell className="text-xs">{DISPLAY_TEXT[displayKey(tool.display)]}</TableCell>
            <TableCell />
            <TableCell />
          </TableRow>
        ))}
    </>
  );
}

/** Shows or hides a group's tool rows. */
function GroupToggle({
  name,
  expanded,
  onToggle,
}: {
  name: string;
  expanded: boolean;
  onToggle: (expanded: boolean) => void;
}) {
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={`${expanded ? "Collapse" : "Expand"} ${name}`}
      aria-expanded={expanded}
      onClick={() => onToggle(!expanded)}
      className="-ml-2"
    >
      <ChevronRightIcon
        className={`transition-transform duration-200 ${expanded ? "rotate-90" : ""}`}
      />
    </Button>
  );
}

/** A source's group header row, then one row per tool its origin offers. */
function SourceRows({
  entry,
  busy,
  act,
}: {
  entry: Entry;
  busy: boolean;
  act: (work: () => Promise<unknown>) => Promise<void>;
}) {
  const { source, name } = entry;
  const [removing, setRemoving] = useState(false);
  const [expanded, setExpanded] = useState(true);
  const used = new Map(source.tools.map((tool) => [tool.toolName, tool]));
  // Tools the agent still uses but the origin no longer offers stay listed so they can be removed.
  const listed: Offered[] = [
    ...entry.offered,
    ...source.tools
      .filter((tool) => !entry.offered.some((o) => o.name === tool.toolName))
      .map((tool) => ({ name: tool.toolName, description: "", summary: "" })),
  ];
  const update = (
    current: AgentTool,
    tool: Offered,
    change: Partial<ReturnType<typeof settings>>,
  ) =>
    act(() =>
      tools.setAgentTool({
        sourceId: source.id,
        toolName: tool.name,
        ...settings(current),
        ...change,
      }),
    );
  return (
    <>
      <TableRow className="bg-muted/40 hover:bg-muted/40">
        <TableCell className="h-14 px-5 py-2">
          {source.toolHostId ? (
            <ZapIcon aria-hidden="true" className="size-5 text-muted-foreground" />
          ) : (
            <ProviderIcon iconUrl={entry.iconUrl} />
          )}
        </TableCell>
        <TableCell colSpan={4}>
          <span className="flex items-center gap-3">
            <GroupToggle name={name} expanded={expanded} onToggle={setExpanded} />
            <Link
              to="/tools/$toolId"
              params={{ toolId: source.connectionId ?? source.toolHostId ?? "" }}
              search={{ kind: source.toolHostId ? "host" : "connection" }}
              className="font-medium hover:underline"
            >
              {name}
            </Link>
            <Badge variant="outline">
              {used.size} of {listed.length} tool{listed.length === 1 ? "" : "s"}
            </Badge>
          </span>
        </TableCell>
        <TableCell className="px-5">
          <span className="flex items-center justify-end">
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={`Remove ${name}`}
              className="text-destructive hover:bg-destructive/10 hover:text-destructive"
              disabled={busy}
              onClick={() => setRemoving(true)}
            >
              <Trash2Icon />
            </Button>
          </span>
        </TableCell>
      </TableRow>
      {(expanded ? listed : []).map((tool) => {
        const current = used.get(tool.name);
        // Overrides are the agent's own; clearing one falls back to the tool's text.
        const editing = !!current && !busy;
        return (
          <TableRow key={tool.name}>
            <TableCell className="px-5">
              <Switch
                aria-label={`Use ${tool.name}`}
                checked={!!current}
                disabled={busy}
                onCheckedChange={(checked) =>
                  void act(() =>
                    checked
                      ? tools.setAgentTool({ sourceId: source.id, toolName: tool.name })
                      : tools.removeAgentTool({ sourceId: source.id, toolName: tool.name }),
                  )
                }
              />
            </TableCell>
            <TableCell className="whitespace-normal">
              <code className="text-xs font-medium">{tool.name}</code>
              <InlineText
                label={`Summary of ${tool.name}`}
                value={current?.summary ?? ""}
                fallback={tool.summary}
                empty="(no summary)"
                maxLength={256}
                editable={editing}
                onSave={(summary) => update(current!, tool, { summary })}
              />
            </TableCell>
            <TableCell className="whitespace-normal">
              <InlineText
                label={`Description of ${tool.name}`}
                value={current?.description ?? ""}
                fallback={tool.description}
                empty="(no description)"
                maxLength={4096}
                multiline
                editable={editing}
                onSave={(description) => update(current!, tool, { description })}
              />
            </TableCell>
            <TableCell>
              {current && (
                <Tabs
                  value={displayKey(current.display)}
                  onValueChange={(value) => {
                    const display = DISPLAY[value as keyof typeof DISPLAY];
                    if (display !== current.display) void update(current, tool, { display });
                  }}
                >
                  <TabsList aria-label={`How ${tool.name} is displayed`}>
                    <TabsTrigger value="full" disabled={busy}>
                      Full
                    </TabsTrigger>
                    <TabsTrigger value="summary" disabled={busy}>
                      Summary
                    </TabsTrigger>
                    <TabsTrigger value="hidden" disabled={busy}>
                      Hidden
                    </TabsTrigger>
                  </TabsList>
                </Tabs>
              )}
            </TableCell>
            <TableCell>
              {current && (
                <Switch
                  aria-label={`Run ${tool.name} in the background`}
                  checked={current.isAsync}
                  disabled={busy}
                  onCheckedChange={(checked) => void update(current, tool, { isAsync: checked })}
                />
              )}
            </TableCell>
            <TableCell />
          </TableRow>
        );
      })}
      <AlertDialog open={removing} onOpenChange={setRemoving}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove {name} from this agent?</AlertDialogTitle>
            <AlertDialogDescription>
              The agent stops using its tools, and your choices for them are lost. The{" "}
              {source.toolHostId ? "tool server" : "connection"} itself stays for other agents.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() =>
                void act(() => tools.removeToolSource({ id: source.id })).then(() =>
                  setRemoving(false),
                )
              }
            >
              Remove
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

/**
 * The agent's own text for a tool, edited in place: the pencil swaps in a field with save and
 * cancel. Shown and edited empty, the tool's own text (`fallback`) applies.
 */
function InlineText({
  label,
  value,
  fallback,
  empty,
  maxLength,
  multiline = false,
  editable,
  onSave,
}: {
  label: string;
  value: string;
  fallback: string;
  empty: string;
  maxLength: number;
  multiline?: boolean;
  editable: boolean;
  onSave: (value: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const shown = value || fallback;
  const save = () => {
    setEditing(false);
    if (draft.trim() !== value) void onSave(draft.trim());
  };
  const cancel = () => {
    setDraft(value);
    setEditing(false);
  };
  if (editing) {
    const field = {
      autoFocus: true,
      "aria-label": label,
      maxLength,
      value: draft,
      placeholder: fallback || empty,
      onKeyDown: (event: React.KeyboardEvent) => {
        if (event.key === "Escape") cancel();
        if (event.key === "Enter" && (!multiline || event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          save();
        }
      },
    };
    return (
      <span className="mt-1 flex items-start gap-1">
        {multiline ? (
          <Textarea
            {...field}
            rows={3}
            className="min-h-0 text-xs"
            onChange={(event) => setDraft(event.currentTarget.value)}
          />
        ) : (
          <Input
            {...field}
            className="h-7 text-xs"
            onChange={(event) => setDraft(event.currentTarget.value)}
          />
        )}
        <IconAction label={`Save ${label.toLowerCase()}`} onClick={save}>
          <CheckIcon />
        </IconAction>
        <IconAction label="Cancel" onClick={cancel}>
          <XIcon />
        </IconAction>
      </span>
    );
  }
  // The pencil follows the last word, so long descriptions are shortened rather than clamped.
  const text = shown || empty;
  const short = multiline && text.length > 160 ? `${text.slice(0, 157).trimEnd()}…` : text;
  return (
    <span className="block text-xs text-muted-foreground" title={short === text ? undefined : text}>
      {short}
      {value && " (edited)"}
      {editable && (
        <button
          type="button"
          aria-label={`Edit ${label.toLowerCase()}`}
          title={`Edit ${label.toLowerCase()}`}
          onClick={() => {
            setDraft(value);
            setEditing(true);
          }}
          className="ml-1 inline cursor-pointer align-baseline text-muted-foreground hover:text-foreground"
        >
          <PencilIcon className="inline size-3 align-[-1px]" />
        </button>
      )}
    </span>
  );
}

function IconAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="inline-flex size-6 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground hover:text-foreground [&_svg]:size-3.5"
    >
      {children}
    </button>
  );
}

/**
 * The ready tool connections and servers the agent does not use yet, searched by the server as
 * the user types. The agent's own sources are few, so they are left out here.
 */
function ChooseExisting({
  open,
  providers,
  used,
  busy,
  onClose,
  onChoose,
}: {
  open: boolean;
  providers: Map<string, Provider>;
  used: string[];
  busy: boolean;
  onClose: () => void;
  onChoose: (origin: Origin) => Promise<void>;
}) {
  return (
    <ChooseDialog
      open={open}
      title="Choose an existing tool"
      description="Your connected tool accounts and tool servers this agent does not use yet. Its tools are added switched off."
      searchLabel="Search tools"
      listLabel="Available tools"
      empty={(search) =>
        search
          ? "No tools match your search."
          : "No other connected tools. Connect a new tool from Connections."
      }
      busy={busy}
      onClose={onClose}
      load={async (search, signal) => {
        const [list, hosts] = await Promise.all([
          loadToolConnections({ search, status: "ready" }, signal),
          toolHosts.listToolHosts({ search, withProvider: false }, { signal }),
        ]);
        return [
          ...list.map((c) => connectionOrigin(c, providers)),
          ...hosts.toolHosts.map(hostOrigin),
        ]
          .filter((origin) => !used.includes(key(origin)))
          .map((origin) => ({
            id: key(origin),
            name: origin.name,
            detail: origin.detail,
            iconUrl: origin.iconUrl,
            value: origin,
          }));
      }}
      reloadKey={used.join()}
      onChoose={(choice) => onChoose(choice.value)}
    />
  );
}
