import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { PencilIcon, PlusIcon } from "lucide-react";
import type { Secret } from "@trytilde/contracts/tilde/management/v1/secrets_pb.js";
import { secrets as client } from "@/client";
import { RemoveButton } from "./remove-button";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const NAME = /^[A-Za-z_][A-Za-z0-9_]{0,127}$/;

/**
 * Connections → Secrets: named values specs reference with `secret.ref("NAME")`, such as a
 * sandbox blueprint's environment variable. Values are write-only. `add` (the `?add=KEY` deep
 * link `tilde deploy` prints for a missing secret) opens a new row with the name filled in and
 * the value focused.
 */
export function SecretsPage({
  add,
  notice,
  onAdded,
}: {
  add?: string;
  /** Shown above the list, such as a link to a team the caller is not in. */
  notice?: string;
  onAdded: () => void;
}) {
  const [list, setList] = useState<Secret[]>();
  const [error, setError] = useState("");
  const [adding, setAdding] = useState<string | undefined>(add);
  const [editing, setEditing] = useState<string>();
  const refresh = useCallback(
    () =>
      client
        .listSecrets({})
        .then(({ secrets }) => setList(secrets))
        .catch((e) => setError(message(e))),
    [],
  );
  useEffect(() => void refresh(), [refresh]);
  useEffect(() => setAdding(add), [add]);
  return (
    <section className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <p className="max-w-2xl text-sm text-muted-foreground">
          Values specs reference by name, such as{" "}
          <code className="text-xs">secret.ref("NPM_TOKEN")</code> in a sandbox blueprint's
          environment. A value is write-only: it is never shown again once saved.
        </p>
        <Button onClick={() => setAdding("")} disabled={adding !== undefined}>
          <PlusIcon /> Add secret
        </Button>
      </div>
      {notice && (
        <p role="status" className="rounded-lg border bg-muted/40 px-4 py-3 text-sm">
          {notice}
        </p>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="overflow-hidden rounded-xl border">
        <Table aria-label="Secrets">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead className="w-64 px-5">Name</TableHead>
              <TableHead>Value</TableHead>
              <TableHead>Used by</TableHead>
              <TableHead className="w-28 text-right">
                <span className="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {list?.map((secret) =>
              editing === secret.id ? (
                <SecretRow
                  key={secret.id}
                  secret={secret}
                  onDone={() => {
                    setEditing(undefined);
                    void refresh();
                  }}
                />
              ) : (
                <SecretView
                  key={secret.id}
                  secret={secret}
                  locked={false}
                  onEdit={() => setEditing(secret.id)}
                  onDelete={() =>
                    client
                      .deleteSecret({ id: secret.id })
                      .then(refresh)
                      .catch((e) => setError(message(e)))
                  }
                />
              ),
            )}
            {adding !== undefined && (
              <SecretRow
                name={adding}
                onDone={(saved) => {
                  setAdding(undefined);
                  if (saved) onAdded();
                  void refresh();
                }}
              />
            )}
            {list && !list.length && adding === undefined && (
              <TableRow>
                <TableCell colSpan={4} className="h-24 text-center text-muted-foreground">
                  No secrets yet.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
    </section>
  );
}

/** A saved secret: no value, its description under it, and the blueprints using it. */
function SecretView({
  secret,
  locked,
  onEdit,
  onDelete,
}: {
  secret: Secret;
  /** Changed only by an administrator (an organization secret seen from a team). */
  locked: boolean;
  onEdit: () => void;
  onDelete: () => Promise<void>;
}) {
  const hidden = secret.blueprintCount - secret.blueprints.length;
  return (
    <TableRow>
      <TableCell className="px-5">
        <code className="text-xs font-medium">{secret.name}</code>
      </TableCell>
      <TableCell>
        <div className="text-muted-foreground">••••••••</div>
        {secret.description && (
          <div className="text-xs text-muted-foreground">{secret.description}</div>
        )}
      </TableCell>
      <TableCell className="text-xs text-muted-foreground">
        {[
          ...secret.blueprints,
          ...(hidden > 0 ? [`${hidden} blueprint${hidden > 1 ? "s" : ""} you cannot see`] : []),
        ].join(", ") || "—"}
      </TableCell>
      <TableCell>
        <span className="flex justify-end gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Replace ${secret.name}`}
            title={
              locked
                ? "Organization secrets are changed by administrators"
                : `Replace ${secret.name}`
            }
            disabled={locked}
            onClick={onEdit}
          >
            <PencilIcon />
          </Button>
          <RemoveButton
            label={`Delete ${secret.name}`}
            title={`Delete ${secret.name}?`}
            description="Specs that reference it are blocked until it is added again."
            confirmLabel="Delete"
            disabled={locked || secret.blueprintCount > 0}
            onConfirm={onDelete}
          />
        </span>
      </TableCell>
    </TableRow>
  );
}

/** A row being added (`name`, possibly empty) or one whose value or description is replaced. */
function SecretRow({
  secret,
  name,
  onDone,
}: {
  secret?: Secret;
  name?: string;
  onDone: (saved: boolean) => void;
}) {
  const form = useForm<{ name: string; value: string; description: string }>({
    defaultValues: {
      name: secret?.name ?? name ?? "",
      value: "",
      description: secret?.description ?? "",
    },
  });
  const [error, setError] = useState("");
  // The name is known (deep link or replace): go straight to the value.
  const focusValue = !!secret || !!name;
  const errors = form.formState.errors;
  return (
    <TableRow className="hover:bg-transparent">
      <TableCell colSpan={4} className="px-5">
        <form
          aria-label={secret ? `Replace ${secret.name}` : "Add secret"}
          className="flex flex-wrap items-start gap-3"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              await client.setSecret({
                // Replacing names the secret itself, so an organization secret stays one.
                id: secret?.id,
                name: values.name.trim(),
                value: values.value || undefined,
                description: values.description,
              });
              onDone(true);
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="w-56 space-y-1">
            <Input
              aria-label="Name"
              placeholder="NPM_TOKEN"
              readOnly={!!secret}
              autoFocus={!focusValue}
              aria-invalid={!!errors.name}
              {...form.register("name", {
                validate: (value) =>
                  NAME.test(value.trim()) ||
                  "Letters, digits and underscores, not starting with a digit.",
              })}
            />
            {errors.name && <p className="text-xs text-destructive">{errors.name.message}</p>}
          </div>
          <div className="min-w-64 flex-1 space-y-1">
            <Input
              aria-label="Value"
              type="password"
              autoComplete="off"
              placeholder={secret ? "New value (empty keeps the current one)" : "Value"}
              autoFocus={focusValue}
              aria-invalid={!!errors.value}
              {...form.register("value", {
                validate: (value) => !!secret || value.length > 0 || "A new secret needs a value.",
              })}
            />
            <Input
              aria-label="Description"
              placeholder="Description (optional)"
              className="h-8 text-xs"
              {...form.register("description", {
                maxLength: { value: 500, message: "At most 500 characters." },
              })}
            />
            {(errors.value || errors.description) && (
              <p className="text-xs text-destructive">
                {errors.value?.message ?? errors.description?.message}
              </p>
            )}
            {error && (
              <p role="alert" className="text-xs text-destructive">
                {error}
              </p>
            )}
          </div>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => onDone(false)}
              disabled={form.formState.isSubmitting}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              Save
            </Button>
          </div>
        </form>
      </TableCell>
    </TableRow>
  );
}
