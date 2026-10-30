import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { Link, useNavigate } from "@tanstack/react-router";
import { PlusIcon } from "lucide-react";
import type { Wiki } from "@trytilde/contracts/tilde/management/v1/wikis_pb.js";
import { wikis as client } from "@/client";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { OwnerLink } from "./linked-agents";
import { CatalogSearchField } from "./tool-catalog";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
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

/** Wikis: Markdown reference pages people write and agents read and edit. */
export function WikisPage() {
  const navigate = useNavigate();
  const registry = useAgentRegistry();
  const [wikis, setWikis] = useState<Wiki[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const list = await client.listWikis({ search: search || undefined }, { signal });
      setWikis(list.wikis);
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
  const open = (wiki: Wiki) =>
    void navigate({ to: "/wikis/$wikiId/pages", params: { wikiId: wiki.id } });
  return (
    <div className="flex flex-col">
      <div
        className="trace-controls flex min-h-10 items-center gap-2 border-b bg-background px-3 py-1.5"
        role="group"
        aria-label="Wiki filter bar"
      >
        <CatalogSearchField label="Search wikis" value={query} onChange={setQuery} />
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap items-center justify-end gap-3">
          <Button className="gap-2" onClick={() => setCreating(true)}>
            <PlusIcon />
            New wiki
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-lg border">
          <Table aria-label="Wikis">
            <TableHeader className="bg-muted/50">
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Owner</TableHead>
                <TableHead className="text-right">Pages</TableHead>
                <TableHead>Agents</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {wikis.length ? (
                wikis.map((wiki) => (
                  <TableRow key={wiki.id}>
                    <TableCell className="h-12 font-medium">
                      <Link
                        to="/wikis/$wikiId/pages"
                        params={{ wikiId: wiki.id }}
                        className="hover:underline"
                      >
                        {wiki.name}
                      </Link>
                      {wiki.description && (
                        <p className="max-w-md truncate text-xs font-normal text-muted-foreground">
                          {wiki.description}
                        </p>
                      )}
                    </TableCell>
                    <TableCell>
                      <OwnerLink
                        agentId={wiki.agentId}
                        name={registry.get(wiki.agentId ?? "")?.name}
                      />
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{wiki.pageCount}</TableCell>
                    <TableCell>
                      <AgentStack
                        ids={wiki.links.map((l) => l.agentId)}
                        registry={registry}
                        onOpen={() => open(wiki)}
                      />
                    </TableCell>
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={4} className="h-24 text-center text-muted-foreground">
                    {loading
                      ? "Loading wikis…"
                      : error
                        ? "Unable to load wikis."
                        : search
                          ? "No wikis match this search."
                          : "No wikis yet."}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </div>
      <CreateWikiDialog open={creating} onClose={() => setCreating(false)} onCreated={open} />
    </div>
  );
}

function CreateWikiDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (wiki: Wiki) => void;
}) {
  const form = useForm({ defaultValues: { name: "", description: "" } });
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
          <DialogTitle>New wiki</DialogTitle>
          <DialogDescription>
            A shared wiki any agent can be linked to. An agent's own wiki is created from its
            Capabilities.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { wiki } = await client.createWiki({
                name: values.name.trim(),
                description: values.description.trim(),
              });
              if (wiki) onCreated(wiki);
              close();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="wiki-name">Name</Label>
            <Input
              id="wiki-name"
              aria-invalid={!!form.formState.errors.name}
              {...form.register("name", { validate: (value) => !!value.trim() })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="wiki-description">Description</Label>
            <Textarea
              id="wiki-description"
              placeholder="What this wiki covers; agents see it."
              {...form.register("description")}
            />
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
              {form.formState.isSubmitting ? "Creating…" : "Create wiki"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
