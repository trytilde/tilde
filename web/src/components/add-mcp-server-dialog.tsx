import { useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate } from "@tanstack/react-router";
import {
  Capability,
  McpCredential,
  OAuthClient,
  OAuthGrant,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, tools } from "@/client";
import { defaultSlug, message } from "./tool-catalog";
import { AccountSetupDialog } from "./tool-account-setup";
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

export type Auth = "none" | "oauth" | "bearer" | "header" | "query";
type Form = { name: string; url: string; auth: Auth; parameter: string; prefix: string };
export const AUTH: Record<Auth, string> = {
  none: "No authentication",
  oauth: "Sign in with OAuth",
  bearer: "Bearer token",
  header: "API key header",
  query: "API key query parameter",
};
const apiKey = JSON.stringify({
  type: "object",
  properties: { api_key: { type: "string", title: "API key", writeOnly: true, minLength: 1 } },
  required: ["api_key"],
  additionalProperties: false,
});

/** The provider definition of a server added by URL: one tool method served over MCP. */
function definition(id: string, values: Form) {
  const url = values.url.trim();
  const mcpServer =
    values.auth === "header"
      ? {
          url,
          credential: McpCredential.HEADER,
          name: values.parameter.trim(),
          prefix: values.prefix,
        }
      : values.auth === "query"
        ? { url, credential: McpCredential.QUERY, name: values.parameter.trim() }
        : {
            url,
            credential: values.auth === "none" ? McpCredential.NONE : McpCredential.BEARER,
          };
  return {
    id,
    name: values.name.trim(),
    iconUrl: "/provider-icons/mcp.svg",
    categories: ["developer_tools"],
    kind: { case: "configured" as const, value: {} },
    connectionTypes: [
      {
        id: values.auth,
        name: AUTH[values.auth],
        capabilities: [Capability.TOOL],
        mcpServer,
        credentialSource:
          values.auth === "oauth"
            ? {
                case: "oauth" as const,
                // Endpoints and the client are discovered from the server at setup.
                value: {
                  grant: OAuthGrant.AUTHORIZATION_CODE,
                  configuration: { client: OAuthClient.DYNAMIC, pkce: true },
                },
              }
            : {
                case: "static" as const,
                value: {
                  schemaJson:
                    values.auth === "none"
                      ? JSON.stringify({
                          type: "object",
                          properties: {},
                          additionalProperties: false,
                        })
                      : apiKey,
                },
              },
      },
    ],
  };
}
/** The server's name as a provider ID no other provider uses. */
async function freeId(name: string) {
  const base = defaultSlug(name);
  for (let n = 1; ; n++) {
    const id = n === 1 ? base : `${base}_${n}`;
    const taken = await connections.getProvider({ id }).then(
      () => true,
      () => false,
    );
    if (!taken) return id;
  }
}

/**
 * Adds an MCP server, then sets up its first account, which continues on the account's page.
 * `onChanged` refreshes the lists.
 */
export function AddMcpServer({
  open,
  auth,
  onOpenChange,
  onChanged,
}: {
  open: boolean;
  /** How accounts authenticate, chosen before the dialog opens. */
  auth: Auth;
  onOpenChange: (open: boolean) => void;
  onChanged: () => void;
}) {
  const navigate = useNavigate();
  const [settingUp, setSettingUp] = useState<Provider>();
  return (
    <>
      <AddMcpServerDialog
        key={auth}
        open={open}
        auth={auth}
        onOpenChange={onOpenChange}
        onAdded={(provider) => {
          onChanged();
          setSettingUp(provider);
        }}
      />
      {settingUp ? (
        <AccountSetupDialog
          provider={settingUp}
          iconId={settingUp.id}
          onStarted={onChanged}
          onComplete={(connectionId) => {
            setSettingUp(undefined);
            // Discover now, so the server lists its tools; a failure shows on the account.
            void tools
              .refreshConnectionTools({ connectionId })
              .catch(() => undefined)
              .finally(
                () =>
                  void navigate({
                    to: "/tools/$toolId",
                    params: { toolId: connectionId },
                    search: { kind: "connection" },
                  }),
              );
          }}
          // Every MCP method has a standard form; nothing continues in a hosted page.
          onCustom={() => setSettingUp(undefined)}
          onClose={() => {
            setSettingUp(undefined);
            onChanged();
          }}
        />
      ) : null}
    </>
  );
}

/**
 * Adds a remote MCP server by URL as a provider of its own: its name, URL and how
 * accounts authenticate. Accounts are then set up like any catalog provider's.
 */
function AddMcpServerDialog({
  open,
  auth,
  onOpenChange,
  onAdded,
}: {
  open: boolean;
  auth: Auth;
  onOpenChange: (open: boolean) => void;
  onAdded: (provider: Provider) => void;
}) {
  const form = useForm<Form>({
    defaultValues: { name: "", url: "", auth, parameter: "", prefix: "" },
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const named = auth === "header" || auth === "query";
  function close() {
    onOpenChange(false);
    setError("");
    form.reset();
  }
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add MCP server</DialogTitle>
          <DialogDescription>
            A remote MCP server over streamable HTTP, whose accounts authenticate with{" "}
            {AUTH[auth].toLowerCase()}. Its tools are discovered when an account connects.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setBusy(true);
            setError("");
            try {
              const { provider } = await connections.registerProvider({
                provider: definition(await freeId(values.name), values),
              });
              close();
              if (provider) onAdded(provider);
            } catch (e) {
              setError(message(e));
            } finally {
              setBusy(false);
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="mcp-server-name">Name</Label>
            <Input id="mcp-server-name" {...form.register("name", { required: true })} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-server-url">Server URL</Label>
            <Input
              id="mcp-server-url"
              type="url"
              placeholder="https://example.com/mcp"
              {...form.register("url", { required: true, pattern: /^https:\/\//i })}
            />
          </div>
          {named && (
            <div className="space-y-2">
              <Label htmlFor="mcp-server-parameter">
                {auth === "header" ? "Header name" : "Parameter name"}
              </Label>
              <Input
                id="mcp-server-parameter"
                placeholder={auth === "header" ? "X-API-Key" : "api_key"}
                {...form.register("parameter", { required: named })}
              />
            </div>
          )}
          {auth === "header" && (
            <div className="space-y-2">
              <Label htmlFor="mcp-server-prefix">Value prefix (optional)</Label>
              <Input id="mcp-server-prefix" placeholder="Token " {...form.register("prefix")} />
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
      </DialogContent>
    </Dialog>
  );
}
