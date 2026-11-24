import { useCallback, useEffect, useMemo, useState } from "react";
import { Controller, FormProvider, useForm } from "react-hook-form";
import { PencilIcon, PlusIcon } from "lucide-react";
import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import type {
  BlueprintEgress,
  SandboxBlueprint,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { connections, sandboxes } from "@/client";
import { ProviderIcon } from "./provider-icon";
import { RemoveButton } from "./remove-button";
import { SchemaForm, schemaDefaults, type JsonSchema, type Option } from "./schema-form";
import { Warning } from "./sandboxes";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { InputGroupAddon } from "@/components/ui/input-group";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
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

/**
 * Every connection with its provider; `egress` keeps those whose provider has egress and whose
 * connection type offers it. Connections are listed unfiltered because ListConnections'
 * capability filter does not cover egress.
 */
async function loadEgressConnections(signal: AbortSignal) {
  const all: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections({ pageSize: 100, pageToken }, { signal });
    all.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  const providers = new Map<string, Provider>();
  await Promise.all(
    [...new Set(all.map((c) => c.providerId))].map((id) =>
      connections.getProvider({ id }, { signal }).then(
        ({ provider }) => provider && providers.set(id, provider),
        () => undefined,
      ),
    ),
  );
  const egress = all.filter((c) =>
    providers
      .get(c.providerId)
      ?.connectionTypes.find((t) => t.id === c.typeId)
      ?.capabilities.includes(Capability.EGRESS),
  );
  return { connections: all, egress, providers };
}
type EgressConnections = Awaited<ReturnType<typeof loadEgressConnections>>;

const SLUG = /^[a-z][a-z0-9_]{0,31}$/;
const schemaOf = (provider?: Provider): JsonSchema | undefined =>
  provider?.egress ? (JSON.parse(provider.egress.configSchemaJson) as JsonSchema) : undefined;

/** The blueprint's egress grants: connections its sandboxes reach through the Tilde gateway. */
export function BlueprintEgressGrants({
  blueprint,
  onChanged,
}: {
  blueprint: SandboxBlueprint;
  onChanged: (blueprint: SandboxBlueprint) => void;
}) {
  const [available, setAvailable] = useState<EgressConnections>();
  // The grant being edited: null adds one; undefined is closed.
  const [editing, setEditing] = useState<BlueprintEgress | null>();
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    loadEgressConnections(abort.signal).then(setAvailable, (e) => {
      if (!abort.signal.aborted) setError(message(e));
    });
    return () => abort.abort();
  }, []);
  const set = (grant: BlueprintEgress, enabled: boolean) =>
    sandboxes
      .setBlueprintEgress({
        blueprintId: blueprint.id,
        slug: grant.slug,
        connectionId: grant.connectionId,
        configJson: grant.configJson,
        enabled,
      })
      .then(({ blueprint }) => {
        setError("");
        if (blueprint) onChanged(blueprint);
      })
      .catch((e) => setError(message(e)));
  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <p className="max-w-2xl text-sm text-muted-foreground">
          Sandboxes reach these connections through the Tilde gateway, which checks every request
          against the grant; the connection's credentials never enter the sandbox.
        </p>
        <Button className="gap-2" disabled={!available} onClick={() => setEditing(null)}>
          <PlusIcon />
          Add egress
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border">
        <Table aria-label="Egress">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead>Slug</TableHead>
              <TableHead>Connection</TableHead>
              <TableHead>Enabled</TableHead>
              <TableHead className="w-24 text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {blueprint.egress.length ? (
              blueprint.egress.map((grant) => {
                const connection = available?.connections.find((c) => c.id === grant.connectionId);
                const provider = connection && available?.providers.get(connection.providerId);
                return (
                  <TableRow key={grant.slug}>
                    <TableCell>
                      <code className="text-xs font-medium">{grant.slug}</code>
                    </TableCell>
                    <TableCell>
                      <span className="flex items-center gap-2">
                        <ProviderIcon iconUrl={provider?.iconUrl} />
                        <span>{connection?.name ?? (available ? "Unknown connection" : "…")}</span>
                      </span>
                    </TableCell>
                    <TableCell>
                      <Switch
                        aria-label={`Enable ${grant.slug}`}
                        checked={grant.enabled}
                        onCheckedChange={(enabled) => void set(grant, enabled)}
                      />
                    </TableCell>
                    <TableCell>
                      <span className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Edit ${grant.slug}`}
                          title={`Edit ${grant.slug}`}
                          disabled={!available}
                          onClick={() => setEditing(grant)}
                        >
                          <PencilIcon />
                        </Button>
                        <RemoveButton
                          label={`Delete ${grant.slug}`}
                          title={`Delete ${grant.slug}?`}
                          description="Sandboxes can no longer reach this connection through the gateway."
                          confirmLabel="Delete"
                          onConfirm={() =>
                            sandboxes
                              .deleteBlueprintEgress({
                                blueprintId: blueprint.id,
                                slug: grant.slug,
                              })
                              .then(({ blueprint }) => blueprint && onChanged(blueprint))
                              .catch((e) => setError(message(e)))
                          }
                        />
                      </span>
                    </TableCell>
                  </TableRow>
                );
              })
            ) : (
              <TableRow>
                <TableCell colSpan={4} className="h-24 text-center text-muted-foreground">
                  No egress. Sandboxes reach no connections through the gateway.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <Dialog open={editing !== undefined} onOpenChange={(open) => !open && setEditing(undefined)}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          {editing !== undefined && available && (
            <EgressForm
              blueprint={blueprint}
              grant={editing ?? undefined}
              available={available}
              onClose={() => setEditing(undefined)}
              onSaved={onChanged}
            />
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}

type EgressValues = {
  connectionId: string;
  slug: string;
  enabled: boolean;
  config: unknown;
};

function EgressForm({
  blueprint,
  grant,
  available,
  onClose,
  onSaved,
}: {
  blueprint: SandboxBlueprint;
  /** The grant being edited; undefined adds one. */
  grant?: BlueprintEgress;
  available: EgressConnections;
  onClose: () => void;
  onSaved: (blueprint: SandboxBlueprint) => void;
}) {
  const providerOf = (connectionId: string) =>
    available.providers.get(
      available.connections.find((c) => c.id === connectionId)?.providerId ?? "",
    );
  const form = useForm<EgressValues>({
    defaultValues: grant
      ? {
          connectionId: grant.connectionId,
          slug: grant.slug,
          enabled: grant.enabled,
          config: schemaDefaults(
            schemaOf(providerOf(grant.connectionId)) ?? {},
            JSON.parse(grant.configJson || "{}"),
          ),
        }
      : { connectionId: "", slug: "", enabled: true, config: {} },
  });
  const [error, setError] = useState("");
  const connectionId = form.watch("connectionId");
  const chosen = available.connections.find((c) => c.id === connectionId);
  const provider = providerOf(connectionId);
  const schema = useMemo(() => schemaOf(provider), [provider]);
  // Each connection is granted once per blueprint.
  const granted = new Set(blueprint.egress.map((g) => g.connectionId));
  const offered = available.egress.filter(
    (c) => !granted.has(c.id) || c.id === grant?.connectionId,
  );
  const loadOptions = useCallback(
    (query: string) =>
      connections
        .runSetupQuery({ connectionId, query, inputJson: "{}", blueprintId: blueprint.id })
        .then(({ outputJson }) => (JSON.parse(outputJson) as { options?: Option[] }).options ?? []),
    [connectionId, blueprint.id],
  );
  return (
    <FormProvider {...form}>
      <DialogHeader>
        <DialogTitle>{grant ? `Edit ${grant.slug}` : "Add egress"}</DialogTitle>
        <DialogDescription>
          Sandboxes reach the connection at{" "}
          <code>/sandboxes/egress/{form.watch("slug") || "slug"}</code> on the gateway, limited to
          what this grant allows.
        </DialogDescription>
      </DialogHeader>
      <form
        className="space-y-4"
        aria-label="Egress"
        onSubmit={form.handleSubmit(async (values) => {
          setError("");
          try {
            const { blueprint: next } = await sandboxes.setBlueprintEgress({
              blueprintId: blueprint.id,
              slug: values.slug,
              connectionId: values.connectionId,
              configJson: JSON.stringify(values.config),
              enabled: values.enabled,
            });
            if (next) onSaved(next);
            onClose();
          } catch (e) {
            setError(message(e));
          }
        })}
      >
        <div className="space-y-2">
          <Label htmlFor="egress-connection">Connection</Label>
          <Controller
            control={form.control}
            name="connectionId"
            rules={{ required: true }}
            render={({ field, fieldState }) => (
              <Combobox
                items={offered}
                value={offered.find((c) => c.id === field.value) ?? null}
                onValueChange={(connection: Connection | null) => {
                  field.onChange(connection?.id ?? "");
                  const next = connection
                    ? available.providers.get(connection.providerId)
                    : undefined;
                  form.setValue("config", schemaDefaults(schemaOf(next) ?? {}));
                  if (next && !form.getFieldState("slug").isDirty)
                    form.setValue("slug", next.id.replace(/[^a-z0-9_]/g, "_"));
                }}
                itemToStringLabel={(connection: Connection) => connection.name}
                itemToStringValue={(connection: Connection) => connection.id}
                disabled={!!grant}
              >
                <ComboboxInput
                  id="egress-connection"
                  placeholder="Search connections"
                  aria-invalid={fieldState.invalid}
                  className="w-full"
                  disabled={!!grant}
                >
                  {chosen && (
                    <InputGroupAddon align="inline-start">
                      <ProviderIcon iconUrl={provider?.iconUrl} />
                    </InputGroupAddon>
                  )}
                </ComboboxInput>
                <ComboboxContent>
                  <ComboboxEmpty>No matching connection.</ComboboxEmpty>
                  <ComboboxList>
                    {(connection: Connection) => (
                      <ComboboxItem key={connection.id} value={connection}>
                        <ProviderIcon
                          iconUrl={available.providers.get(connection.providerId)?.iconUrl}
                        />
                        <span>{connection.name}</span>
                      </ComboboxItem>
                    )}
                  </ComboboxList>
                </ComboboxContent>
              </Combobox>
            )}
          />
          {!offered.length && (
            <p className="text-xs text-muted-foreground">
              No connection that offers egress, such as GitHub, GitLab or Entire, is left to grant.
            </p>
          )}
        </div>
        <div className="space-y-2">
          <Label htmlFor="egress-slug">Slug</Label>
          <Input
            id="egress-slug"
            readOnly={!!grant}
            placeholder="github"
            aria-invalid={!!form.formState.errors.slug}
            {...form.register("slug", {
              validate: (value) =>
                !SLUG.test(value)
                  ? "Lowercase letters, digits and underscores, starting with a letter (at most 32)."
                  : !grant && blueprint.egress.some((g) => g.slug === value)
                    ? "Another grant uses this slug."
                    : true,
            })}
          />
          {form.formState.errors.slug && (
            <p role="alert" className="text-xs text-destructive">
              {form.formState.errors.slug.message}
            </p>
          )}
        </div>
        {chosen && chosen.typeId !== "github_app" && (
          <Warning>
            This connection's token cannot be narrowed to the grant, so only the gateway's checks
            limit what sandboxes do with it.
            {chosen.providerId === "github" &&
              " A GitHub App connection also mints a token limited to the grant."}
          </Warning>
        )}
        {schema && (
          <SchemaForm key={connectionId} schema={schema} name="config" loadOptions={loadOptions} />
        )}
        <Controller
          control={form.control}
          name="enabled"
          render={({ field }) => (
            <div className="flex items-center justify-between gap-4">
              <Label htmlFor="egress-enabled">Enabled</Label>
              <Switch id="egress-enabled" checked={field.value} onCheckedChange={field.onChange} />
            </div>
          )}
        />
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
            disabled={form.formState.isSubmitting}
          >
            Cancel
          </Button>
          <Button type="submit" disabled={form.formState.isSubmitting}>
            Save
          </Button>
        </DialogFooter>
      </form>
    </FormProvider>
  );
}
