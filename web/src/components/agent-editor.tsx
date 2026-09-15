import { AgentLogs } from "@/features/logs/agent-logs";
import { AgentTracing } from "@/features/tracing/agent-tracing";
import { DashboardNavigation } from "./dashboard-breadcrumbs";
import { InlineSaving, type InlineSavingState } from "./inline-saving";
import { AgentDeployment } from "@/components/agent-deployment";
import { AgentIam } from "@/components/agent-iam";
import { AgentAvatar } from "./agent-avatar";
import { AgentTargetPicker } from "./agent-target-picker";
import { AgentConnections } from "./agent-connections";
import { randomUUID } from "@/lib/browser-crypto";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { clone, create } from "@bufbuild/protobuf";
import {
  type Agent,
  AgentConcurrencyPolicy,
  type Capabilities,
  CapabilitiesSchema,
  TargetPermissionSchema,
  BinaryPermission,
  TargetSelection,
} from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { agents, deployments } from "@/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { ArrowLeftIcon, PencilIcon } from "lucide-react";
import { useForm, NativeSelect } from "@trytilde/connection-ui";

type BinaryKey = "agentsCreate" | "threadRead" | "workRead" | "workWrite" | "runUpdate";
type TargetKey =
  | "agentsRead"
  | "agentsUpdate"
  | "agentsDelete"
  | "agentsInvoke"
  | "agentsGrantCapabilities"
  | "toolsInvoke";
type CapabilityOption = { label: string; description: string } & (
  | { key: BinaryKey; targeted: false }
  | { key: TargetKey; targeted: true }
);
const capabilityOptions: readonly CapabilityOption[] = [
  {
    key: "agentsRead",
    label: "Read agents",
    targeted: true,
    description: "Discover agents and view their details.",
  },
  {
    key: "agentsCreate",
    label: "Create agents",
    targeted: false,
    description: "Register new agents.",
  },
  {
    key: "agentsUpdate",
    label: "Update agents",
    targeted: true,
    description: "Change existing agents’ settings.",
  },
  {
    key: "agentsDelete",
    label: "Delete agents",
    targeted: true,
    description: "Remove agents from the registry.",
  },
  {
    key: "agentsInvoke",
    label: "Invoke agents in this thread",
    targeted: true,
    description: "Ask other agents to work in the current thread.",
  },
  {
    key: "agentsGrantCapabilities",
    label: "Grant capabilities",
    targeted: true,
    description: "Manage the permissions assigned to other agents.",
  },
  {
    key: "threadRead",
    label: "Read current thread",
    targeted: false,
    description: "Read the conversation within the current invocation.",
  },
  {
    key: "workRead",
    label: "Read goals and tasks",
    targeted: false,
    description: "View goals and tasks within the current invocation.",
  },
  {
    key: "workWrite",
    label: "Manage goals and tasks",
    targeted: false,
    description: "Create and update goals and tasks within the current invocation.",
  },
  {
    key: "runUpdate",
    label: "Update own run",
    targeted: false,
    description: "Report progress and update the current run.",
  },
  {
    key: "toolsInvoke",
    label: "Use provider tools",
    targeted: true,
    description: "Call tools exposed by connected providers.",
  },
];

export type AgentTab =
  | "capabilities"
  | "chat-providers"
  | "iam"
  | "deployment"
  | "tracing"
  | "logs";

export function AgentEditor({
  agent,
  onClose,
  onSaved,
  tab,
  onTabChange,
  onNameSaved,
}: {
  agent: Agent | null;
  tab?: AgentTab;
  onTabChange?: (tab: AgentTab) => void;
  onNameSaved?: (name: string) => void;
  onClose: () => void;
  onSaved: (created: boolean) => void;
}) {
  const [capabilities, setCapabilities] = useState(() =>
    create(CapabilitiesSchema, agent?.capabilities),
  );
  const currentCapabilities = useRef(capabilities);
  const [capabilitySaving, setCapabilitySaving] = useState(false);
  const capabilityPending = useRef(false);
  const [capabilityFeedback, setCapabilityFeedback] = useState<
    Partial<
      Record<CapabilityOption["key"], { state: InlineSavingState; attempt: number; error?: string }>
    >
  >({});
  const capabilityAttempt = useRef(0);

  async function persistCapabilities(key: CapabilityOption["key"], next: Capabilities) {
    if (capabilityPending.current) throw new Error("A capability change is already saving.");
    const previous = currentCapabilities.current;
    currentCapabilities.current = next;
    setCapabilities(next);
    if (!agent) return; // Creation submits the initial permissions with the required registration fields.
    const attempt = ++capabilityAttempt.current;
    setCapabilityFeedback((current) => ({ ...current, [key]: { state: "saving", attempt } }));
    capabilityPending.current = true;
    setCapabilitySaving(true);
    try {
      const response = await agents.updateAgent({ id: agent.id, capabilities: next });
      const saved = response.agent?.capabilities ?? next;
      currentCapabilities.current = saved;
      setCapabilities(saved);
      setCapabilityFeedback((current) => ({ ...current, [key]: { state: "success", attempt } }));
      metadataSaved.current = true;
    } catch (error) {
      currentCapabilities.current = previous;
      setCapabilities(previous);
      setCapabilityFeedback((current) => ({
        ...current,
        [key]: {
          state: "error",
          attempt,
          error: error instanceof Error ? error.message : "Unable to save capability.",
        },
      }));
      throw error;
    } finally {
      capabilityPending.current = false;
      setCapabilitySaving(false);
    }
  }

  async function changeCapability(
    option: CapabilityOption,
    mode: BinaryPermission | TargetSelection,
    ids: string[] = [],
  ) {
    const next = clone(CapabilitiesSchema, currentCapabilities.current);
    if (option.targeted)
      next[option.key] = create(TargetPermissionSchema, { mode: mode as TargetSelection, ids });
    else next[option.key] = mode as BinaryPermission;
    await persistCapabilities(option.key, next);
  }
  const policyForm = useForm<{ policy: AgentConcurrencyPolicy }>({
    defaultValues: { policy: agent?.concurrencyPolicy || AgentConcurrencyPolicy.QUEUE },
  });
  const [policyFeedback, setPolicyFeedback] = useState<{
    state: InlineSavingState;
    attempt: number;
    error?: string;
  }>({ state: "idle", attempt: 0 });
  async function changePolicy(policy: AgentConcurrencyPolicy) {
    const previous = policyForm.getValues("policy");
    policyForm.setValue("policy", policy);
    if (!agent) return;
    setPolicyFeedback((current) => ({ state: "saving", attempt: current.attempt + 1 }));
    try {
      const response = await agents.updateAgent({ id: agent.id, concurrencyPolicy: policy });
      policyForm.setValue("policy", response.agent?.concurrencyPolicy || policy);
      metadataSaved.current = true;
      setPolicyFeedback((current) => ({ ...current, state: "success" }));
    } catch (error) {
      policyForm.setValue("policy", previous);
      setPolicyFeedback((current) => ({
        ...current,
        state: "error",
        error: error instanceof Error ? error.message : "Unable to save message policy.",
      }));
    }
  }
  const [name, setName] = useState(agent?.name ?? "");
  const [creationMode, setCreationMode] = useState("gateway");
  const [endpoint, setEndpoint] = useState(agent?.endpointUrl ?? "");
  const [signingKey, setSigningKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const id = useRef(agent?.id ?? randomUUID());
  const [createdAgent, setCreatedAgent] = useState<Agent | null>(null);
  const [avatarFile, setAvatarFile] = useState<File | null>(null);
  const [avatarUrl, setAvatarUrl] = useState(agent?.avatarUrl);
  const [preview, setPreview] = useState<string>();
  const [avatarSaving, setAvatarSaving] = useState(false);
  const [avatarError, setAvatarError] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!avatarFile) {
      setPreview(undefined);
      return;
    }
    const url = URL.createObjectURL(avatarFile);
    setPreview(url);
    return () => URL.revokeObjectURL(url);
  }, [avatarFile]);
  const locked = saving || !!createdAgent || capabilitySaving || policyFeedback.state === "saving";
  async function uploadAvatar(target: string, file: File) {
    setAvatarSaving(true);
    setAvatarError("");
    try {
      const response = await agents.uploadAgentAvatar({
        id: target,
        content: new Uint8Array(await file.arrayBuffer()),
        mediaType: file.type,
      });
      setAvatarUrl(response.agent?.avatarUrl);
      setAvatarFile(null);
      metadataSaved.current = true;
    } finally {
      setAvatarSaving(false);
    }
  }
  const metadataSaved = useRef(false);
  const [metadataSaving, setMetadataSaving] = useState(false);
  const leave = () => {
    if (capabilityPending.current) return;
    if (createdAgent) onSaved(true);
    else if (metadataSaved.current) onSaved(false);
    else onClose();
  };
  async function saveMetadata(field: "name" | "endpointUrl", value: string) {
    if (!agent) return;
    setMetadataSaving(true);
    try {
      const response = await agents.updateAgent({ id: agent.id, [field]: value });
      if (field === "name") {
        const savedName = response.agent?.name ?? value.trim();
        setName(savedName);
        onNameSaved?.(savedName);
      } else setEndpoint(response.agent?.endpointUrl ?? value);
      metadataSaved.current = true;
    } finally {
      setMetadataSaving(false);
    }
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (agent) return;
    setSaving(true);
    setError("");
    try {
      {
        let created = createdAgent;
        if (!created) {
          const response =
            creationMode === "sidecar"
              ? await deployments.createSidecarAgent({
                  concurrencyPolicy: policyForm.getValues("policy"),
                  capabilities,
                  id: id.current,
                  name,
                  webhookSigningKey: signingKey,
                })
              : await agents.createAgent({
                  concurrencyPolicy: policyForm.getValues("policy"),
                  capabilities,
                  id: id.current,
                  name,
                  endpointUrl: endpoint,
                  webhookSigningKey: signingKey,
                });
          created = response.agent ?? null;
          if (!created) throw new Error("Agent creation returned no agent.");
          setCreatedAgent(created);
        }
        if (avatarFile) await uploadAvatar(created.id, avatarFile);
      }
      setSigningKey("");
      onSaved(!agent);
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to save agent.");
    } finally {
      setSaving(false);
    }
  }
  const form = (
    <form id="agent-settings" onSubmit={save} className="grid gap-5">
      {createdAgent && (
        <p role="status">Agent created. Finish uploading the avatar or return to the registry.</p>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {!agent && (
        <div className="grid gap-2">
          <Label htmlFor="agent-key">Webhook signing key</Label>
          <Input
            id="agent-key"
            type="password"
            value={signingKey}
            onChange={(event) => setSigningKey(event.target.value)}
            required
            minLength={32}
            maxLength={1024}
            pattern="[!-~]+"
            autoComplete="new-password"
            disabled={locked}
            aria-describedby="agent-key-help"
          />
          <p id="agent-key-help" className="text-xs text-muted-foreground">
            Supply your own securely generated key and configure the same key in your agent. It is
            never returned after saving.
          </p>
        </div>
      )}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Label htmlFor="agent-message-policy">New messages</Label>
          <InlineSaving
            state={policyFeedback.state}
            label="Message policy"
            resetKey={policyFeedback.attempt}
            error={policyFeedback.error}
          />
        </div>
        <NativeSelect
          id="agent-message-policy"
          aria-describedby="agent-message-policy-help"
          {...policyForm.register("policy")}
          value={policyForm.watch("policy")}
          disabled={locked}
          onChange={(event) =>
            void changePolicy(Number(event.target.value) as AgentConcurrencyPolicy)
          }
        >
          <option value={AgentConcurrencyPolicy.QUEUE}>Queue</option>
          <option value={AgentConcurrencyPolicy.INTERRUPT}>Interrupt</option>
          <option value={AgentConcurrencyPolicy.QUEUE_AND_BATCH}>Queue and batch</option>
        </NativeSelect>
        <p id="agent-message-policy-help" className="text-xs text-muted-foreground">
          {policyForm.watch("policy") === AgentConcurrencyPolicy.INTERRUPT
            ? "Cancel the current response and start a fresh response for new messages."
            : policyForm.watch("policy") === AgentConcurrencyPolicy.QUEUE_AND_BATCH
              ? "Finish the current response, then handle pending messages together."
              : "Finish the current response, then handle each pending message separately."}
        </p>
      </div>
      <fieldset className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-6 lg:grid-cols-[max-content_minmax(0,1fr)_max-content_minmax(0,1fr)] lg:gap-x-6">
        <legend className={agent ? "sr-only" : "mb-2 font-medium"}>Capabilities</legend>
        {capabilityOptions.map((option) => {
          const { key, label, description } = option;
          const mode = option.targeted
            ? (capabilities[option.key]?.mode ?? TargetSelection.NONE)
            : capabilities[option.key] || BinaryPermission.NO;
          const no = option.targeted ? TargetSelection.NONE : BinaryPermission.NO;
          const yes = option.targeted ? TargetSelection.ALL : BinaryPermission.YES;
          return (
            <div
              key={key}
              className="col-span-2 grid min-w-0 grid-cols-subgrid items-start gap-y-3"
            >
              <Tabs
                value={mode}
                onValueChange={(value) => {
                  if (typeof value === "number")
                    void changeCapability(option, value).catch(() => {});
                }}
              >
                <TabsList aria-label={label} aria-describedby={`cap-${key}-description`}>
                  <TabsTrigger value={no} disabled={locked}>
                    {option.targeted ? "None" : "No"}
                  </TabsTrigger>
                  <TabsTrigger value={yes} disabled={locked}>
                    {option.targeted ? "All" : "Yes"}
                  </TabsTrigger>
                  {option.targeted && (
                    <TabsTrigger value={TargetSelection.SELECTED} disabled={locked}>
                      Selected
                    </TabsTrigger>
                  )}
                </TabsList>
              </Tabs>
              <div className="min-w-0 pt-1">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-medium">{label}</h3>
                  <InlineSaving
                    state={capabilityFeedback[key]?.state ?? "idle"}
                    label={label}
                    resetKey={capabilityFeedback[key]?.attempt}
                    error={capabilityFeedback[key]?.error}
                  />
                </div>
                <p
                  id={`cap-${key}-description`}
                  className="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {description}
                </p>
              </div>
              {option.targeted &&
                mode === TargetSelection.SELECTED &&
                (option.key === "toolsInvoke" ? (
                  <ToolTargetsInput
                    label={label}
                    ids={capabilities[option.key]?.ids ?? []}
                    disabled={locked}
                    onCommit={(ids) => changeCapability(option, TargetSelection.SELECTED, ids)}
                  />
                ) : (
                  <AgentTargetPicker
                    label={label}
                    value={capabilities[option.key]?.ids ?? []}
                    disabled={locked}
                    onConfirm={(ids) => changeCapability(option, TargetSelection.SELECTED, ids)}
                  />
                ))}
            </div>
          );
        })}
      </fieldset>
      {!agent && (
        <div className="flex justify-end">
          <Button
            type="submit"
            disabled={saving || metadataSaving || avatarSaving || !name.trim() || !endpoint.trim()}
          >
            {saving
              ? "Saving…"
              : createdAgent
                ? avatarFile
                  ? "Retry avatar upload"
                  : "Finish"
                : "Create agent"}
          </Button>
        </div>
      )}
    </form>
  );
  return (
    <section
      className="flex flex-1 flex-col px-4 py-6 lg:px-8"
      aria-label={agent ? "Edit agent" : "Create agent"}
    >
      <div className="mx-auto grid w-full max-w-6xl gap-8">
        <header className="grid gap-3">
          {!agent && (
            <Button
              type="button"
              variant="ghost"
              className="mb-2 w-fit -ml-2"
              disabled={saving || metadataSaving || avatarSaving || capabilitySaving}
              onClick={leave}
            >
              <ArrowLeftIcon /> Back to agents
            </Button>
          )}
          <div className="flex min-w-0 items-center gap-6">
            <div className="shrink-0">
              <button
                type="button"
                className="group/avatar relative isolate grid size-20 shrink-0 place-items-center rounded-full border bg-muted/30 p-2 focus-visible:outline-2 focus-visible:outline-ring"
                aria-label="Upload agent avatar"
                disabled={saving || avatarSaving}
                onClick={() => fileInput.current?.click()}
              >
                <AgentAvatar
                  agent={{
                    id: id.current,
                    avatarSeed: agent?.avatarSeed,
                    avatarUrl: preview || avatarUrl,
                  }}
                  className="size-full!"
                  animated
                />
                <span className="pointer-events-none absolute inset-0 z-10 grid place-items-center rounded-full bg-black/50 text-white opacity-0 transition-opacity group-hover/avatar:opacity-100 group-focus-visible/avatar:opacity-100">
                  <PencilIcon className="size-5" />
                </span>
              </button>
              <input
                ref={fileInput}
                type="file"
                accept="image/png,image/jpeg,image/gif,image/webp"
                className="sr-only"
                aria-label="Avatar image"
                disabled={saving || avatarSaving}
                onChange={async (event) => {
                  const file = event.target.files?.[0];
                  event.target.value = "";
                  if (!file) return;
                  setAvatarError("");
                  if (
                    !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(file.type) ||
                    file.size > 5 * 1024 * 1024 ||
                    !file.size
                  ) {
                    setAvatarError("Choose a PNG, JPEG, GIF or WebP image of at most 5 MiB.");
                    return;
                  }
                  setAvatarFile(file);
                  const target = agent?.id ?? createdAgent?.id;
                  if (target)
                    try {
                      await uploadAvatar(target, file);
                    } catch (error) {
                      setAvatarError(
                        error instanceof Error ? error.message : "Unable to upload avatar.",
                      );
                    }
                }}
              />
            </div>
            <div className="grid min-w-0 flex-1 gap-3">
              {agent ? (
                <>
                  <InlineAgentField
                    label="Agent name"
                    value={name}
                    heading
                    disabled={saving || metadataSaving}
                    onSave={(value) => saveMetadata("name", value)}
                  />
                </>
              ) : (
                <>
                  <h1 className="sr-only">Create agent</h1>
                  <Input
                    form="agent-settings"
                    aria-label="Agent name"
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                    required
                    maxLength={200}
                    disabled={locked}
                    autoFocus
                    placeholder="Agent name"
                    className="h-11 border-transparent bg-transparent p-0 text-3xl font-semibold shadow-none md:text-3xl"
                  />
                  <Tabs
                    value={creationMode}
                    onValueChange={(value) => setCreationMode(String(value))}
                  >
                    <TabsList aria-label="Deployment mode">
                      <TabsTrigger value="gateway">Gateway</TabsTrigger>
                      <TabsTrigger value="sidecar">Sidecar</TabsTrigger>
                    </TabsList>
                  </Tabs>
                  {creationMode === "gateway" && (
                    <Input
                      form="agent-settings"
                      aria-label="Agent endpoint"
                      type="url"
                      value={endpoint}
                      onChange={(event) => setEndpoint(event.target.value)}
                      required
                      disabled={locked}
                      placeholder="https://agent.example.com"
                      className="border-transparent bg-transparent p-0 shadow-none"
                    />
                  )}
                </>
              )}
            </div>
          </div>
          {avatarSaving && (
            <p role="status" className="text-sm text-muted-foreground">
              Uploading avatar…
            </p>
          )}
          {avatarError && (
            <p role="alert" className="text-sm text-destructive">
              {avatarError}
            </p>
          )}
        </header>
        {agent ? (
          <Tabs
            defaultValue="capabilities"
            value={tab}
            onValueChange={(value) => onTabChange?.(value as AgentTab)}
          >
            <DashboardNavigation>
              <TabsList aria-label="Agent settings">
                <TabsTrigger value="capabilities">Capabilities</TabsTrigger>
                <TabsTrigger value="chat-providers">Chat providers</TabsTrigger>
                <TabsTrigger value="iam">IAM</TabsTrigger>
                <TabsTrigger value="tracing">Tracing</TabsTrigger>
                <TabsTrigger value="logs">Logs</TabsTrigger>
                <TabsTrigger value="deployment">Deployment</TabsTrigger>
              </TabsList>
            </DashboardNavigation>
            <TabsContent value="capabilities" keepMounted>
              {form}
            </TabsContent>
            <TabsContent value="chat-providers" keepMounted>
              <AgentConnections agentId={agent.id} />
            </TabsContent>
            <TabsContent value="deployment">
              <AgentDeployment agentId={agent.id} paused={agent.paused} />
            </TabsContent>
            <TabsContent value="logs">
              {tab === "logs" && <AgentLogs agentId={agent.id} />}
            </TabsContent>
            <TabsContent value="tracing">
              {tab === "tracing" && <AgentTracing agentId={agent.id} />}
            </TabsContent>
            <TabsContent value="iam" keepMounted>
              <AgentIam key={agent.id} agentId={agent.id} />
            </TabsContent>
          </Tabs>
        ) : (
          form
        )}
      </div>
    </section>
  );
}

/** Header edits persist only the selected field, independently of capability drafts. */
function InlineAgentField({
  label,
  value,
  heading = false,
  disabled,
  onSave,
}: {
  label: string;
  value: string;
  heading?: boolean;
  disabled: boolean;
  onSave: (value: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const form = useForm<{ value: string }>({ defaultValues: { value } });
  const typography = heading
    ? "text-3xl font-semibold tracking-tight md:text-3xl"
    : "text-sm text-muted-foreground md:text-sm";
  const error = form.formState.errors.value?.message;
  const errorId = heading ? "agent-name-error" : "agent-endpoint-error";
  const submit = form.handleSubmit(async ({ value }) => {
    try {
      await onSave(value);
      setEditing(false);
    } catch (error) {
      form.setError("value", {
        message: error instanceof Error ? error.message : "Unable to save this field.",
      });
    }
  });
  return (
    <div className="min-w-0">
      {editing ? (
        <form key="editing" onSubmit={submit} className="flex min-w-0 items-center gap-3">
          <Input
            aria-label={label}
            aria-invalid={!!error}
            aria-describedby={error ? errorId : undefined}
            type={heading ? "text" : "url"}
            required
            autoFocus
            disabled={disabled}
            maxLength={heading ? 200 : undefined}
            style={{ width: `${Math.max(heading ? 12 : 24, form.watch("value").length + 1)}ch` }}
            className={`min-w-0 max-w-full border-transparent bg-transparent p-0 shadow-none focus-visible:border-transparent focus-visible:ring-0 dark:bg-transparent ${heading ? "h-11" : "h-8"} ${typography}`}
            {...form.register("value", {
              validate: (value) => !!value.trim() || `${label} is required.`,
            })}
            onKeyDown={(event) => {
              if (event.key === "Escape" && !disabled) {
                event.preventDefault();
                form.reset({ value });
                setEditing(false);
              }
            }}
          />
          <Button type="submit" size="sm" disabled={disabled || form.formState.isSubmitting}>
            {form.formState.isSubmitting ? "Saving…" : "Save"}
          </Button>
        </form>
      ) : (
        <div key="display" className="flex min-w-0 items-center gap-3">
          {heading ? (
            <h1 className={`min-w-0 break-words ${typography}`}>{value}</h1>
          ) : (
            <p className={`min-w-0 break-all ${typography}`}>{value || "Endpoint required"}</p>
          )}
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={`Edit ${label.toLowerCase()}`}
            disabled={disabled}
            onClick={(event) => {
              event.preventDefault();
              form.reset({ value });
              setEditing(true);
            }}
          >
            <PencilIcon />
          </Button>
        </div>
      )}
      {error && editing && (
        <p id={errorId} role="alert" className="mt-1 text-sm text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

/** Text tool targets commit on blur or Enter; partial comma-separated edits never reach the API. */
function ToolTargetsInput({
  label,
  ids,
  disabled,
  onCommit,
}: {
  label: string;
  ids: string[];
  disabled: boolean;
  onCommit: (ids: string[]) => Promise<void>;
}) {
  const [text, setText] = useState(ids.join(", "));
  useEffect(() => setText(ids.join(", ")), [ids]);
  return (
    <Input
      className="col-span-full"
      aria-label={`${label} targets`}
      placeholder="Tool names, separated by commas"
      value={text}
      disabled={disabled}
      onChange={(event) => setText(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          event.currentTarget.blur();
        }
      }}
      onBlur={() => {
        const next = [
          ...new Set(
            text
              .split(",")
              .map((name) => name.trim())
              .filter(Boolean),
          ),
        ];
        if (next.join(",") !== ids.join(","))
          void onCommit(next).catch(() => setText(ids.join(", ")));
      }}
    />
  );
}
