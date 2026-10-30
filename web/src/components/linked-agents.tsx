import type { ReactNode } from "react";
import { Controller, useForm } from "react-hook-form";
import { Link } from "@tanstack/react-router";
import { NativeSelect } from "@trytilde/connection-ui";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { RemoveButton } from "./remove-button";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

/** Read (1) or read-write (2): memory bank and wiki access share these values. */
export type Access = 1 | 2;
export const ACCESS_LABELS: Record<Access, string> = { 1: "Read", 2: "Read and write" };
export type AgentLink = { agentId: string; access: number };

/**
 * The agents linked to a bank or wiki and their access, with unlinking and a form to link
 * another. `own` is the agent whose own bank or wiki it is: nothing else can be linked to it.
 */
export function LinkedAgentsTable({
  links,
  own,
  registry,
  editable,
  hint,
  unlinkDescription,
  onSet,
  onUnlink,
}: {
  links: AgentLink[];
  own?: string;
  registry: Map<string, Agent>;
  editable: boolean;
  hint: string;
  unlinkDescription: string;
  onSet: (agentId: string, access: Access) => Promise<void>;
  onUnlink: (agentId: string) => Promise<void>;
}) {
  const form = useForm({ defaultValues: { agentId: "", access: "1" } });
  const name = (agentId: string) => registry.get(agentId)?.name ?? "Agent";
  const unlinked = [...registry.values()].filter((a) => !links.some((l) => l.agentId === a.id));
  return (
    <section className="grid gap-3 border-t pt-6" aria-label="Linked agents">
      <div>
        <h3 className="text-sm font-medium">Linked agents</h3>
        <p className="text-xs text-muted-foreground">{hint}</p>
      </div>
      <div className="overflow-hidden rounded-lg border">
        <Table aria-label="Linked agents">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead>Agent</TableHead>
              <TableHead>Access</TableHead>
              <TableHead className="w-16 text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {links.length ? (
              links.map(({ agentId, access }) => (
                <TableRow key={agentId}>
                  <TableCell>
                    <Link
                      to="/agent/$agentId/capabilities"
                      params={{ agentId }}
                      className="hover:underline"
                    >
                      {name(agentId)}
                    </Link>
                  </TableCell>
                  <TableCell>
                    {agentId === own ? (
                      "Read and write (its own)"
                    ) : !editable ? (
                      (ACCESS_LABELS[access as Access] ?? "Read")
                    ) : (
                      <NativeSelect
                        aria-label={`Access of ${name(agentId)}`}
                        className="w-auto"
                        value={String(access)}
                        onChange={(event) =>
                          void onSet(agentId, Number(event.target.value) as Access)
                        }
                      >
                        {Object.entries(ACCESS_LABELS).map(([value, label]) => (
                          <option key={value} value={value}>
                            {label}
                          </option>
                        ))}
                      </NativeSelect>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    {agentId !== own && editable && (
                      <RemoveButton
                        label={`Unlink ${name(agentId)}`}
                        title="Unlink this agent?"
                        description={unlinkDescription}
                        confirmLabel="Unlink"
                        onConfirm={() => onUnlink(agentId)}
                      />
                    )}
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={3} className="h-16 text-center text-muted-foreground">
                  No agents linked.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      {!own && editable && (
        <form
          className="flex flex-wrap items-end gap-3"
          aria-label="Link an agent"
          onSubmit={form.handleSubmit(async (values) => {
            await onSet(values.agentId, Number(values.access) as Access);
            form.reset();
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="link-agent">Agent</Label>
            <Controller
              control={form.control}
              name="agentId"
              rules={{ required: true }}
              render={({ field }) => (
                <NativeSelect id="link-agent" {...field}>
                  <option value="">Choose an agent</option>
                  {unlinked.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </NativeSelect>
              )}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="link-access">Access</Label>
            <NativeSelect id="link-access" {...form.register("access")}>
              {Object.entries(ACCESS_LABELS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </NativeSelect>
          </div>
          <Button type="submit" variant="outline" disabled={form.formState.isSubmitting}>
            Link agent
          </Button>
        </form>
      )}
    </section>
  );
}

/** "Shared", or a link to the agent whose own bank or wiki it is. */
export function OwnerLink({ agentId, name }: { agentId?: string; name?: string }) {
  if (!agentId) return <span className="text-muted-foreground">Shared</span>;
  return (
    <Link to="/agent/$agentId/capabilities" params={{ agentId }} className="hover:underline">
      {name ?? "Agent"}
    </Link>
  );
}

/** One shared bank or wiki an agent may be linked to. */
export type Resource = { id: string; name: string; detail?: string };
/**
 * The shared banks or wikis an agent is linked to, with their access, unlinking, and a picker to
 * link another. It sits inside the agent form, so the picker submits through its button rather
 * than a nested form.
 */
export function LinkedResources({
  noun,
  plural,
  resources,
  links,
  locked,
  open,
  empty,
  unlinkDescription,
  onSet,
  onUnlink,
}: {
  /** "bank" or "wiki", for labels. */
  noun: string;
  /** The row's label, e.g. "Other banks". */
  plural: string;
  /** Every shared bank or wiki. */
  resources: Resource[];
  links: { id: string; access: number }[];
  locked: boolean;
  /** Links a resource to its page. */
  open: (resource: Resource) => ReactNode;
  /** Shown when there is nothing to link, e.g. a link to create one. */
  empty: ReactNode;
  unlinkDescription: string;
  onSet: (id: string, access: Access) => void;
  onUnlink: (id: string) => void;
}) {
  const adding = useForm({ defaultValues: { id: "", access: "1" } });
  const find = (id: string) => resources.find((r) => r.id === id) ?? { id, name: noun };
  const unlinked = resources.filter((r) => !links.some((l) => l.id === r.id));
  return (
    <div className="grid gap-2 sm:grid-cols-[10rem_minmax(0,1fr)]">
      <Label className="sm:pt-1.5">{plural}</Label>
      <ul className="grid gap-2" aria-label={plural}>
        {links.map((link) => {
          const resource = find(link.id);
          return (
            <li key={link.id} className="flex flex-wrap items-center gap-2">
              <span className="min-w-40 text-sm">{open(resource)}</span>
              <NativeSelect
                aria-label={`Access to ${resource.name}`}
                className="w-auto"
                value={String(link.access)}
                disabled={locked}
                onChange={(event) => onSet(link.id, Number(event.target.value) as Access)}
              >
                {Object.entries(ACCESS_LABELS).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </NativeSelect>
              <RemoveButton
                label={`Unlink ${resource.name}`}
                title={`Unlink ${resource.name}?`}
                description={unlinkDescription}
                confirmLabel="Unlink"
                disabled={locked}
                onConfirm={() => onUnlink(link.id)}
              />
            </li>
          );
        })}
        {unlinked.length ? (
          <li className="flex flex-wrap items-center gap-2">
            <Controller
              control={adding.control}
              name="id"
              rules={{ required: true }}
              render={({ field }) => (
                <NativeSelect
                  aria-label={`Shared ${noun} to link`}
                  className="w-auto"
                  disabled={locked}
                  {...field}
                >
                  <option value="">Choose a shared {noun}</option>
                  {unlinked.map((r) => (
                    <option key={r.id} value={r.id}>
                      {r.detail ? `${r.name} (${r.detail})` : r.name}
                    </option>
                  ))}
                </NativeSelect>
              )}
            />
            <NativeSelect
              aria-label={`Access to the ${noun} to link`}
              className="w-auto"
              disabled={locked}
              {...adding.register("access")}
            >
              {Object.entries(ACCESS_LABELS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </NativeSelect>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={locked || adding.formState.isSubmitting}
              onClick={adding.handleSubmit((values) => {
                onSet(values.id, Number(values.access) as Access);
                adding.reset();
              })}
            >
              Link {noun}
            </Button>
          </li>
        ) : (
          !links.length && <li className="text-xs text-muted-foreground">{empty}</li>
        )}
      </ul>
    </div>
  );
}
