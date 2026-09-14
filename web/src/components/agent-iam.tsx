import { agentAccess } from "@/client";
import { ChannelAccessMode, IdentityType } from "@trytilde/contracts/tilde/types/v1/access_pb.js";
import { Fragment, useCallback, useEffect, useState } from "react";
import { ChevronDownIcon, ChevronRightIcon, PlusIcon, ShieldCheckIcon } from "lucide-react";
import { useForm, NativeSelect } from "@trytilde/connection-ui";
import { randomUUID } from "@/lib/browser-crypto";
import { ProviderIcon } from "./provider-icon";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./ui/dialog";

type AccessMode = "private" | "public" | "disabled";
type ProviderRoute = {
  id: string;
  providerId: string;
  iconUrl: string;
  name: string;
  account: string;
  mode: AccessMode;
  identityTypes: IdentityType[];
  initialValue?: string;
  initialType?: IdentityType;
  ready: boolean;
  verificationSupported: boolean;
  supportsTemplate: boolean;
  verificationInstructions: string;
};
type Identity = {
  id: string;
  routeId: string;
  value: string;
  identityType: IdentityType;
  verified: boolean;
  allowed: boolean;
  verificationStatus?: string;
};
/** Persisted, provider-scoped access. Verification is distinct from the per-agent allow flag. */
export function AgentIam({ agentId }: { agentId: string }) {
  const [routes, setRoutes] = useState<ProviderRoute[]>([]);
  const [identities, setIdentities] = useState<Identity[]>([]);
  const [view, setView] = useState("provider");
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [adding, setAdding] = useState<ProviderRoute | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const loadedRoutes: ProviderRoute[] = [];
      const loadedIdentities: Identity[] = [];
      let pageToken = "";
      do {
        const page = await agentAccess.listChannelAccess(
          { agentId, pageToken, pageSize: 100 },
          { signal },
        );
        loadedRoutes.push(
          ...page.routes.map((route): ProviderRoute => ({
            id: route.connectionId,
            providerId: route.providerId,
            iconUrl: route.iconUrl ?? "",
            name: route.providerName,
            account: route.accountName,
            mode:
              route.mode === ChannelAccessMode.PUBLIC
                ? "public"
                : route.mode === ChannelAccessMode.PRIVATE
                  ? "private"
                  : "disabled",
            identityTypes: route.identityTypes,
            ready: route.connectionStatus === "ready",
            verificationSupported: route.verificationSupported,
            supportsTemplate: route.supportsTemplate,
            verificationInstructions: route.verificationInstructions,
          })),
        );
        pageToken = page.nextPageToken;
      } while (pageToken);
      do {
        const page = await agentAccess.listChannelIdentities(
          { agentId, pageToken, pageSize: 100 },
          { signal },
        );
        loadedIdentities.push(
          ...page.identities.map((identity) => ({
            id: identity.id,
            routeId: identity.connectionId,
            value: identity.value,
            identityType: identity.identityType,
            verified: !!identity.verifiedAt,
            allowed: identity.allowed,
            verificationStatus: identity.verificationStatus,
          })),
        );
        pageToken = page.nextPageToken;
      } while (pageToken);
      if (!signal?.aborted) {
        setRoutes(loadedRoutes);
        setIdentities(loadedIdentities);
      }
    },
    [agentId],
  );
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    setError("");
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted)
          setError(e instanceof Error ? e.message : "Unable to load access policies.");
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh, attempt]);
  useEffect(() => {
    if (
      busy ||
      !identities.some(
        (identity) =>
          !identity.verified &&
          ["pending", "delivered"].includes(identity.verificationStatus ?? ""),
      )
    )
      return;
    const abort = new AbortController();
    const timer = setTimeout(() => {
      void refresh(abort.signal).catch(() => {});
    }, 3000);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [identities, busy, refresh]);
  async function act(operation: () => Promise<void>) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await operation();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unable to update access.");
    } finally {
      setBusy(false);
    }
  }
  function mode(route: ProviderRoute, value: string | null) {
    const mode =
      value === "private"
        ? ChannelAccessMode.PRIVATE
        : value === "public"
          ? ChannelAccessMode.PUBLIC
          : ChannelAccessMode.DISABLED;
    void act(async () => {
      await agentAccess.setChannelAccess({ agentId, connectionId: route.id, mode });
    });
  }
  function toggle(identity: Identity, allowed: boolean) {
    void act(async () => {
      await agentAccess.setIdentityAccess({
        agentId,
        connectionId: identity.routeId,
        identityId: identity.id,
        allowed,
      });
    });
  }
  async function requestVerification(
    route: ProviderRoute,
    value: string,
    identityType: IdentityType,
    templateName?: string,
    templateLanguage?: string,
  ) {
    await act(async () => {
      const response = await agentAccess.requestIdentityVerification({
        id: randomUUID(),
        agentId,
        connectionId: route.id,
        value,
        identityType,
        templateName,
        templateLanguage,
      });
      setAdding(null);
      if (response.status === "delivered")
        setNotice(
          `Verification message sent to ${value}. Access remains disabled until they approve.`,
        );
      else
        setError(
          "Verification could not be delivered. Check the connection and recipient, then retry. WhatsApp may require an approved message template.",
        );
    });
  }
  function access(identity: Identity, route: ProviderRoute) {
    if (route.mode === "public") return <Badge variant="secondary">Allowed · public</Badge>;
    if (route.mode === "disabled") return <Badge variant="outline">Route disabled</Badge>;
    return (
      <div className="flex items-center justify-end gap-2">
        <span className="text-xs text-muted-foreground">
          {!identity.verified ? "Verify first" : identity.allowed ? "Allowed" : "Disabled"}
        </span>
        <Switch
          aria-label={`Allow ${identity.value} through ${route.name}`}
          checked={identity.verified && identity.allowed}
          disabled={busy || !identity.verified}
          onCheckedChange={(checked) => toggle(identity, checked)}
        />
      </div>
    );
  }
  function verificationBadge(identity: Identity, route: ProviderRoute) {
    return identity.verified ? (
      <Badge variant="outline" className="border-success/20 text-success">
        <ShieldCheckIcon />
        Verified
      </Badge>
    ) : (
      <Badge
        variant="outline"
        className="cursor-pointer border-warning/20 text-warning"
        render={
          <button
            type="button"
            aria-label={`Verify ${identity.value}`}
            disabled={busy || !route.ready || !route.verificationSupported}
            onClick={() =>
              setAdding({
                ...route,
                initialValue: identity.value,
                initialType: identity.identityType,
              })
            }
          />
        }
      >
        {identity.verificationStatus === "failed"
          ? "Delivery failed"
          : identity.verificationStatus === "expired"
            ? "Expired"
            : ["pending", "delivered"].includes(identity.verificationStatus ?? "")
              ? "Pending verification"
              : "Unverified"}
      </Badge>
    );
  }

  return (
    <section className="space-y-5" aria-label="Agent IAM">
      <div className="space-y-2">
        <h3 className="font-medium">Who can invoke this agent?</h3>
        <p className="text-sm text-muted-foreground">
          Manage access through each connected chat provider.
        </p>
      </div>
      {loading && (
        <p role="status" className="text-sm text-muted-foreground">
          Loading access policies…
        </p>
      )}
      {error && (
        <div
          role="alert"
          className="flex items-center justify-between gap-3 text-sm text-destructive"
        >
          <p>{error}</p>
          <Button variant="outline" onClick={() => setAttempt((value) => value + 1)}>
            Refresh
          </Button>
        </div>
      )}
      <Tabs value={view} onValueChange={(value) => setView(String(value))}>
        <TabsList aria-label="IAM view">
          <TabsTrigger value="provider">View by chat provider</TabsTrigger>
          <TabsTrigger value="user">View by user</TabsTrigger>
        </TabsList>
      </Tabs>
      {notice && (
        <p role="status" className="text-sm text-muted-foreground">
          {notice}
        </p>
      )}
      <div className="overflow-hidden rounded-xl border">
        <Table aria-label={view === "provider" ? "Chat provider access" : "User access"}>
          <TableHeader className="bg-background">
            <TableRow>
              <TableHead className="h-11 px-5">Identity</TableHead>
              {view === "user" && <TableHead>Account</TableHead>}
              <TableHead className="h-11 px-5 text-right">Access</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {view === "provider"
              ? routes.map((route) => {
                  const members = identities.filter((identity) => identity.routeId === route.id);
                  const expanded = route.mode === "private" && !collapsed.has(route.id);
                  return (
                    <Fragment key={route.id}>
                      <TableRow className="bg-muted/40 hover:bg-muted/40">
                        <TableCell colSpan={2} className="px-3 py-2.5">
                          <div className="flex flex-wrap items-center justify-between gap-3">
                            <div className="flex min-w-0 items-center gap-2">
                              {route.mode === "private" ? (
                                <Button
                                  variant="ghost"
                                  size="icon-sm"
                                  aria-label={`${expanded ? "Collapse" : "Expand"} ${route.account}`}
                                  aria-expanded={expanded}
                                  onClick={() =>
                                    setCollapsed((current) => {
                                      const next = new Set(current);
                                      if (next.has(route.id)) next.delete(route.id);
                                      else next.add(route.id);
                                      return next;
                                    })
                                  }
                                >
                                  {expanded ? <ChevronDownIcon /> : <ChevronRightIcon />}
                                </Button>
                              ) : (
                                <span className="w-7" />
                              )}
                              <ProviderIcon iconUrl={route.iconUrl} />
                              <strong className="truncate text-sm font-semibold">
                                {route.account}
                              </strong>
                              {route.mode === "private" && (
                                <span className="rounded-md border bg-background px-1.5 text-xs tabular-nums text-muted-foreground">
                                  {members.length}
                                </span>
                              )}
                            </div>
                            <div className="ml-auto flex items-center gap-3">
                              {route.mode === "private" && (
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  disabled={busy || !route.ready}
                                  onClick={() => setAdding(route)}
                                >
                                  <PlusIcon />
                                  Add identity
                                </Button>
                              )}
                              <Tabs
                                value={route.mode}
                                onValueChange={(value) => mode(route, value)}
                              >
                                <TabsList aria-label={`${route.name} access mode`}>
                                  {route.verificationSupported && (
                                    <TabsTrigger value="private" disabled={busy}>
                                      Private
                                    </TabsTrigger>
                                  )}
                                  <TabsTrigger
                                    value="public"
                                    disabled={busy}
                                    title="Anyone can invoke; every sender still gets an identity record."
                                  >
                                    Public
                                  </TabsTrigger>
                                  <TabsTrigger
                                    value="disabled"
                                    disabled={busy}
                                    title="No traffic is forwarded to this agent."
                                  >
                                    Disabled
                                  </TabsTrigger>
                                </TabsList>
                              </Tabs>
                            </div>
                          </div>
                        </TableCell>
                      </TableRow>
                      {expanded &&
                        members.map((identity) => (
                          <TableRow key={identity.id}>
                            <TableCell className="h-14 py-2 pl-14 pr-5">
                              <div className="flex items-center gap-3">
                                <span className="text-sm tabular-nums">{identity.value}</span>
                                {verificationBadge(identity, route)}
                              </div>
                            </TableCell>
                            <TableCell className="px-5 text-right">
                              {access(identity, route)}
                            </TableCell>
                          </TableRow>
                        ))}
                      {expanded && members.length === 0 && (
                        <TableRow>
                          <TableCell
                            colSpan={2}
                            className="h-14 pl-14 text-sm text-muted-foreground"
                          >
                            No identities added.
                          </TableCell>
                        </TableRow>
                      )}
                    </Fragment>
                  );
                })
              : [...identities]
                  .sort((a, b) => a.value.localeCompare(b.value))
                  .map((identity) => {
                    const route = routes.find((item) => item.id === identity.routeId)!;
                    return (
                      <TableRow key={identity.id}>
                        <TableCell className="h-14 px-5 py-2">
                          <div className="flex items-center gap-3">
                            <span className="text-sm tabular-nums">{identity.value}</span>
                            {verificationBadge(identity, route)}
                          </div>
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            <ProviderIcon iconUrl={route.iconUrl} />
                            {route.account}
                          </div>
                        </TableCell>
                        <TableCell className="px-5 text-right">{access(identity, route)}</TableCell>
                      </TableRow>
                    );
                  })}
          </TableBody>
        </Table>
        {!loading && routes.length === 0 && (
          <p className="p-5 text-sm text-muted-foreground">Add a chat provider to manage access.</p>
        )}
      </div>
      <Dialog
        open={!!adding}
        onOpenChange={(open) => {
          if (!open) setAdding(null);
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Add a {adding?.name} identity</DialogTitle>
            <DialogDescription>
              The recipient approves access using a verification link. No login required.
            </DialogDescription>
          </DialogHeader>
          {adding && (
            <AddIdentity
              key={`${adding.id}:${adding.initialValue ?? "new"}`}
              route={adding}
              busy={busy}
              onSubmit={(value, identityType, template, language) =>
                requestVerification(adding, value, identityType, template, language)
              }
            />
          )}
        </DialogContent>
      </Dialog>
    </section>
  );
}

const identityLabels = {
  [IdentityType.EMAIL]: "Email",
  [IdentityType.PHONE_NUMBER]: "Phone number",
  [IdentityType.USERNAME]: "Username",
  [IdentityType.UNSPECIFIED]: "Identity",
};
function AddIdentity({
  route,
  busy,
  onSubmit,
}: {
  route: ProviderRoute;
  busy: boolean;
  onSubmit: (
    value: string,
    identityType: IdentityType,
    template?: string,
    language?: string,
  ) => Promise<void>;
}) {
  const {
    register,
    handleSubmit,
    watch,
    formState: { errors },
  } = useForm<{ value: string; identityType: string; template: string; language: string }>({
    defaultValues: {
      value: route.initialValue ?? "",
      identityType: String(route.initialType ?? route.identityTypes[0]),
      language: "en",
    },
  });
  const identityType = Number(watch("identityType")) as IdentityType;
  return (
    <form
      className="space-y-4"
      onSubmit={handleSubmit(({ value, identityType, template, language }) =>
        onSubmit(
          value,
          Number(identityType),
          template?.trim() || undefined,
          template?.trim() ? language?.trim() || "en" : undefined,
        ),
      )}
    >
      {route.identityTypes.length > 1 && (
        <div className="space-y-2">
          <Label htmlFor="iam-type">Identity type</Label>
          <NativeSelect id="iam-type" {...register("identityType")}>
            {route.identityTypes.map((type) => (
              <option key={type} value={type}>
                {identityLabels[type]}
              </option>
            ))}
          </NativeSelect>
        </div>
      )}
      <div className="space-y-2">
        <Label htmlFor="iam-value">{identityLabels[identityType]}</Label>
        <Input
          id="iam-value"
          type={
            identityType === IdentityType.EMAIL
              ? "email"
              : identityType === IdentityType.PHONE_NUMBER
                ? "tel"
                : "text"
          }
          {...register("value", { validate: (value) => !!value || "Enter an identity." })}
        />
        {errors.value && (
          <p role="alert" className="text-sm text-destructive">
            {errors.value.message}
          </p>
        )}
      </div>
      {route.supportsTemplate && (
        <div className="space-y-3 rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">{route.verificationInstructions}</p>
          <Label htmlFor="iam-template">Approved template (optional)</Label>
          <Input id="iam-template" {...register("template")} />
          <Label htmlFor="iam-language">Template language</Label>
          <Input id="iam-language" {...register("language")} />
        </div>
      )}
      <Button type="submit" disabled={busy}>
        {busy ? "Sending…" : "Send verification"}
      </Button>
    </form>
  );
}
