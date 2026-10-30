import { useCallback, useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { Link, useNavigate } from "@tanstack/react-router";
import { PlusIcon, SlidersHorizontalIcon } from "lucide-react";
import { NativeSelect } from "@trytilde/connection-ui";
import { Capability, type Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import {
  MemoryBankScope,
  MemoryKind,
  type EmbeddingModel,
  type EmbeddingSettings,
  type MemoryBank,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import { connections, memory } from "@/client";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { OwnerLink } from "./linked-agents";
import { CatalogSearchField } from "./tool-catalog";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type Scope = Exclude<MemoryBankScope, MemoryBankScope.UNSPECIFIED>;
/** How each bank scope reads. */
export const SCOPES: Record<Scope, { label: string; description: string }> = {
  [MemoryBankScope.SHARED]: {
    label: "Shared",
    description: "Agents read and write every memory in the bank.",
  },
  [MemoryBankScope.PER_PERSON]: {
    label: "Per person",
    description:
      "An agent reads and writes only the memories of the verified person it acts for, plus facts people wrote for everyone.",
  },
};
// Unspecified scope means SHARED.
export const scopeOf = (scope: MemoryBankScope): Scope => scope || MemoryBankScope.SHARED;

export const KINDS: Record<Exclude<MemoryKind, MemoryKind.UNSPECIFIED>, string> = {
  [MemoryKind.PROFILE]: "Profile",
  [MemoryKind.PREFERENCES]: "Preferences",
  [MemoryKind.ENTITIES]: "Entities",
  [MemoryKind.EVENTS]: "Events",
  [MemoryKind.CASES]: "Cases",
  [MemoryKind.PATTERNS]: "Patterns",
  [MemoryKind.TOOLS]: "Tools",
  [MemoryKind.SKILLS]: "Skills",
};

/** Memory banks: what agents remember, and the embedding model recall uses. */
export function MemoryPage() {
  const navigate = useNavigate();
  const registry = useAgentRegistry();
  const [banks, setBanks] = useState<MemoryBank[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [settings, setSettings] = useState(false);
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const list = await memory.listMemoryBanks({ search: search || undefined }, { signal });
      setBanks(list.banks);
      setError("");
    },
    [search],
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
  const open = (bank: MemoryBank) =>
    void navigate({ to: "/memory/$bankId/memories", params: { bankId: bank.id } });
  return (
    <div className="flex flex-col">
      <div
        className="trace-controls flex min-h-10 items-center gap-2 border-b bg-background px-3 py-1.5"
        role="group"
        aria-label="Memory bank filter bar"
      >
        <CatalogSearchField label="Search memory banks" value={query} onChange={setQuery} />
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap items-center justify-end gap-3">
          <Button variant="outline" className="gap-2" onClick={() => setSettings(true)}>
            <SlidersHorizontalIcon />
            Embedding settings
          </Button>
          <Button className="gap-2" onClick={() => setCreating(true)}>
            <PlusIcon />
            New bank
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-lg border">
          <Table aria-label="Memory banks">
            <TableHeader className="bg-muted/50">
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Owner</TableHead>
                <TableHead>Scope</TableHead>
                <TableHead className="text-right">Memories</TableHead>
                <TableHead className="text-right">Waiting to embed</TableHead>
                <TableHead>Agents</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {banks.length ? (
                banks.map((bank) => (
                  <TableRow key={bank.id}>
                    <TableCell className="h-12 font-medium">
                      <Link
                        to="/memory/$bankId/memories"
                        params={{ bankId: bank.id }}
                        className="hover:underline"
                      >
                        {bank.name}
                      </Link>
                    </TableCell>
                    <TableCell>
                      <OwnerLink
                        agentId={bank.agentId}
                        name={registry.get(bank.agentId ?? "")?.name}
                      />
                    </TableCell>
                    <TableCell>{SCOPES[scopeOf(bank.scope)].label}</TableCell>
                    <TableCell className="text-right tabular-nums">{bank.memoryCount}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {bank.pendingEmbeddings || "—"}
                    </TableCell>
                    <TableCell>
                      <AgentStack
                        ids={bank.links.map((l) => l.agentId)}
                        registry={registry}
                        onOpen={() => open(bank)}
                      />
                    </TableCell>
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                    {loading
                      ? "Loading memory banks…"
                      : error
                        ? "Unable to load memory banks."
                        : search
                          ? "No memory banks match this search."
                          : "No memory banks yet."}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </div>
      <CreateBankDialog open={creating} onClose={() => setCreating(false)} onCreated={open} />
      <EmbeddingSettingsDialog open={settings} onClose={() => setSettings(false)} />
    </div>
  );
}

/** Choosing a bank's scope, which is fixed once the bank exists. */
export function ScopeChoice({
  value,
  onChange,
  labelledBy,
}: {
  value: string;
  onChange: (value: string) => void;
  labelledBy: string;
}) {
  return (
    <RadioGroup
      aria-labelledby={labelledBy}
      value={value}
      onValueChange={(next) => onChange(String(next))}
      className="grid gap-2 sm:grid-cols-2"
    >
      {Object.entries(SCOPES).map(([scope, { label, description }]) => (
        <label
          key={scope}
          className="flex cursor-pointer items-start gap-3 rounded-lg border p-3 has-data-checked:border-primary has-data-checked:bg-primary/5"
        >
          <RadioGroupItem value={scope} className="mt-0.5" />
          <span className="space-y-1">
            <span className="block text-sm font-medium">{label}</span>
            <span className="block text-xs text-muted-foreground">{description}</span>
          </span>
        </label>
      ))}
    </RadioGroup>
  );
}

type BankValues = { name: string; description: string; scope: string };
function CreateBankDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (bank: MemoryBank) => void;
}) {
  const form = useForm<BankValues>({
    defaultValues: { name: "", description: "", scope: String(MemoryBankScope.SHARED) },
  });
  const [error, setError] = useState("");
  function close() {
    onClose();
    form.reset();
    setError("");
  }
  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>New memory bank</DialogTitle>
          <DialogDescription>
            A shared bank any agent can be linked to. An agent's own bank is created from its
            Capabilities.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { bank } = await memory.createMemoryBank({
                name: values.name.trim(),
                description: values.description.trim(),
                scope: Number(values.scope) as MemoryBankScope,
              });
              if (bank) onCreated(bank);
              close();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="bank-name">Name</Label>
            <Input
              id="bank-name"
              aria-invalid={!!form.formState.errors.name}
              {...form.register("name", { validate: (value) => !!value.trim() })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="bank-description">Description</Label>
            <Textarea
              id="bank-description"
              placeholder="What belongs in this bank; agents see it."
              {...form.register("description")}
            />
          </div>
          <div className="space-y-2">
            <Label id="bank-scope">Scope</Label>
            <Controller
              control={form.control}
              name="scope"
              render={({ field }) => (
                <ScopeChoice
                  value={field.value}
                  onChange={field.onChange}
                  labelledBy="bank-scope"
                />
              )}
            />
            <p className="text-xs text-muted-foreground">The scope cannot change later.</p>
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              Create bank
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Ready inference connections, which embedding and synthesis settings may use. */
export async function inferenceConnections(signal?: AbortSignal) {
  const all: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections(
      { capability: Capability.INFERENCE, status: "ready", pageSize: 100, pageToken },
      { signal },
    );
    all.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return all;
}

type SettingsValues = { connectionId: string; model: string; dimensions: string };
/**
 * The embedding model recall uses for similarity, through an inference connection. Saving embeds
 * a probe first, so a model that fails or returns another dimension is refused, then every
 * memory is embedded again.
 */
function EmbeddingSettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [current, setCurrent] = useState<EmbeddingSettings | null>();
  const [available, setAvailable] = useState<Connection[]>([]);
  const [models, setModels] = useState<EmbeddingModel[]>([]);
  const [error, setError] = useState("");
  const [clearing, setClearing] = useState(false);
  const form = useForm<SettingsValues>({
    defaultValues: { connectionId: "", model: "", dimensions: "1536" },
  });
  const connectionId = form.watch("connectionId");
  useEffect(() => {
    if (!open) return;
    const abort = new AbortController();
    void Promise.all([
      memory.getEmbeddingSettings({}, { signal: abort.signal }),
      inferenceConnections(abort.signal),
    ])
      .then(([{ settings }, found]) => {
        setCurrent(settings ?? null);
        setAvailable(found);
        form.reset({
          connectionId: settings?.connectionId ?? found[0]?.id ?? "",
          model: settings?.model ?? "",
          dimensions: String(settings?.dimensions || 1536),
        });
      })
      .catch((e) => !abort.signal.aborted && setError(message(e)));
    return () => abort.abort();
  }, [open, form]);
  useEffect(() => {
    if (!connectionId) return setModels([]);
    const abort = new AbortController();
    memory
      .listEmbeddingModels({ connectionId }, { signal: abort.signal })
      .then(({ models }) => setModels(models))
      .catch(() => !abort.signal.aborted && setModels([]));
    return () => abort.abort();
  }, [connectionId]);
  function close() {
    onClose();
    setError("");
  }
  async function clear() {
    setClearing(true);
    setError("");
    try {
      await memory.clearEmbeddingSettings({});
      setCurrent(null);
    } catch (e) {
      setError(message(e));
    } finally {
      setClearing(false);
    }
  }
  const named = (id: string) => available.find((c) => c.id === id)?.name ?? "Unknown connection";
  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Embedding settings</DialogTitle>
          <DialogDescription>
            Recall matches memories by their words, and by meaning once an embedding model is set.
          </DialogDescription>
        </DialogHeader>
        <p className="text-sm" role="status">
          {current === undefined
            ? "Loading…"
            : current
              ? `Embedding with ${current.model} (${current.dimensions} dimensions) through ${named(current.connectionId)}.`
              : "No embedding model: memories are recalled by their words only."}
        </p>
        <form
          className="space-y-4"
          aria-label="Embedding settings"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { settings } = await memory.setEmbeddingSettings({
                settings: {
                  connectionId: values.connectionId,
                  model: values.model.trim(),
                  dimensions: Number(values.dimensions),
                },
              });
              setCurrent(settings ?? null);
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="embedding-connection">Inference connection</Label>
            <NativeSelect
              id="embedding-connection"
              {...form.register("connectionId", { required: true })}
            >
              {!available.length && <option value="">No ready inference connection</option>}
              {available.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </NativeSelect>
            <p className="text-xs text-muted-foreground">
              OpenAI, Azure OpenAI, Vercel AI Gateway, Baseten, Voyage or Cohere.
            </p>
          </div>
          <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_max-content]">
            <div className="space-y-2">
              <Label htmlFor="embedding-model">Model</Label>
              <Input
                id="embedding-model"
                list="embedding-models"
                autoComplete="off"
                aria-invalid={!!form.formState.errors.model}
                {...form.register("model", {
                  validate: (value) => !!value.trim(),
                  onChange: (event) => {
                    const known = models.find((m) => m.model === event.target.value);
                    if (known?.dimensions) form.setValue("dimensions", String(known.dimensions));
                  },
                })}
              />
              <datalist id="embedding-models">
                {models.map((m) => (
                  <option key={m.model} value={m.model} />
                ))}
              </datalist>
            </div>
            <div className="space-y-2">
              <Label htmlFor="embedding-dimensions">Dimensions</Label>
              <NativeSelect id="embedding-dimensions" {...form.register("dimensions")}>
                <option value="1024">1024</option>
                <option value="1536">1536</option>
              </NativeSelect>
            </div>
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            {current && (
              <Button
                type="button"
                variant="outline"
                className="mr-auto"
                disabled={clearing}
                onClick={() => void clear()}
              >
                Clear
              </Button>
            )}
            <Button type="button" variant="outline" onClick={close}>
              Close
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting || !available.length}>
              {form.formState.isSubmitting ? "Checking the model…" : "Save"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
