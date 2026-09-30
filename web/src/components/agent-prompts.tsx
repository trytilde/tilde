import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { ArrowLeftIcon, CodeIcon } from "lucide-react";
import {
  PromptFormat,
  type Prompt,
  type PromptVersion,
  type PromptVersionUsage,
} from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { prompts } from "@/client";
import { date } from "@/features/tracing/format";
import { dollars } from "./inference-usage-chart";
import { PromptFormatBadge } from "./deployment-contents";
import { ExpandableText } from "./expandable-text";
import { OpenFile, type PaneFile } from "./file-pane";
import { SkillExplorer } from "./skill-explorer";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { TildeLoader } from "@/components/loading-screen";

const message = (error: unknown, fallback: string) =>
  error instanceof Error ? error.message : fallback;
const when = (timestamp: { seconds: bigint } | undefined) =>
  timestamp ? date(new Date(Number(timestamp.seconds) * 1000).toISOString()) : "—";
/** How a variable is written in a version's template format. */
const variable = (format: PromptFormat, name: string) =>
  format === PromptFormat.BRACES ? `{${name}}` : `{{${name}}}`;
const text = (path: string, content: string): PaneFile => ({
  path,
  content,
  binary: false,
  mediaType: "",
  sizeBytes: 0n,
  downloadUrl: "",
});
/** A version's content as files: the template, each section and the config the code declared. */
export function promptFiles(version: PromptVersion): PaneFile[] {
  return [
    // A dynamic prompt's template is the source of the function that renders it.
    text(version.format === PromptFormat.DYNAMIC ? "function" : "template.md", version.template),
    ...version.sections.map((s) => text(`sections/${s.name}.md`, s.content)),
    text("config.json", prettyJson(version.config)),
  ];
}
function prettyJson(text: string) {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}
const COLUMNS = 2;

/** Prompts `tilde deploy` found in the agent's code; read-only, since the code owns them. */
export function AgentPrompts({ agentId }: { agentId: string }) {
  const [list, setList] = useState<Prompt[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // The open prompt lives in the URL so a deployment's contents can link to it.
  const open = (useSearch({ strict: false }) as { prompt?: string }).prompt;
  const navigate = useNavigate();
  const setOpen = (prompt: string | undefined) =>
    void navigate({
      to: "/agent/$agentId/prompts",
      params: { agentId },
      search: prompt ? { prompt } : {},
    });
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    setError("");
    void prompts
      .listPrompts({ agentId }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) setList(response.prompts);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load prompts."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [agentId]);
  if (open) return <PromptDetail id={open} onBack={() => setOpen(undefined)} />;
  return (
    <section className="space-y-4" aria-label="Prompts">
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border" aria-busy={loading}>
        <Table aria-label="Prompts">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead className="px-4">Name</TableHead>
              <TableHead className="px-4">Prompt</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={COLUMNS} className="h-24">
                  <TildeLoader className="py-0" />
                </TableCell>
              </TableRow>
            ) : list.length === 0 ? (
              <TableRow>
                <TableCell colSpan={COLUMNS} className="h-24 text-center text-muted-foreground">
                  No prompts registered yet. They appear when <code>tilde deploy</code> registers a
                  deployment whose code declares one.
                </TableCell>
              </TableRow>
            ) : (
              list.map((prompt) => (
                <TableRow
                  key={prompt.id}
                  className="cursor-pointer"
                  onClick={() => setOpen(prompt.id)}
                >
                  <TableCell className="w-64 px-4 align-top">
                    <span className="flex items-center gap-2">
                      <button
                        type="button"
                        className="cursor-pointer font-mono text-sm font-medium hover:underline"
                      >
                        {prompt.name}
                      </button>
                      {prompt.latest && (
                        <Badge
                          variant="secondary"
                          className="bg-muted font-normal text-muted-foreground"
                        >
                          v{prompt.latest.number}
                        </Badge>
                      )}
                    </span>
                  </TableCell>
                  <TableCell className="px-4 align-top whitespace-normal">
                    <ExpandableText
                      text={prompt.latest?.template || "—"}
                      className="max-w-2xl text-muted-foreground"
                    />
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </section>
  );
}

/**
 * One prompt's latest version, laid out like a skill: its files in a read-only explorer and the
 * open file in the rich or code editor. Changing a prompt means changing the code and deploying.
 */
function PromptDetail({ id, onBack }: { id: string; onBack: () => void }) {
  const [prompt, setPrompt] = useState<Prompt>();
  const [version, setVersion] = useState<PromptVersion>();
  const [usage, setUsage] = useState<PromptVersionUsage>();
  const [path, setPath] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    void prompts
      .getPrompt({ id }, { signal: abort.signal })
      .then((response) => {
        if (abort.signal.aborted) return;
        const latest = response.versions[0];
        setPrompt(response.prompt);
        setVersion(latest);
        setUsage(response.usage.find((u) => u.versionId === latest?.id));
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load the prompt."));
      });
    return () => abort.abort();
  }, [id]);
  const files = useMemo(() => (version ? promptFiles(version) : []), [version]);
  const file = files.find((f) => f.path === path) ?? files[0];
  const noop = () => {};
  return (
    <section
      className="flex min-h-0 flex-1"
      aria-label={prompt ? `Prompt ${prompt.name}` : "Prompt"}
    >
      <aside className="flex w-64 shrink-0 flex-col gap-4 border-r pt-4 lg:pt-6">
        {version && (
          <div className="space-y-1 px-4 lg:px-6">
            <div className="flex flex-wrap items-center gap-2">
              <CodeIcon className="size-4" />
              <span className="text-sm font-medium">Code</span>
              <Badge
                variant="outline"
                title="Prompts follow the agent's code and change only when it is deployed."
              >
                Read-only
              </Badge>
            </div>
            {version.origin && (
              <p className="m-0 font-mono text-xs break-all text-muted-foreground">
                {version.origin}
              </p>
            )}
          </div>
        )}
        {prompt && version && (
          <SkillExplorer
            title={prompt.name}
            files={files}
            folders={[]}
            selected={file?.path ?? ""}
            readOnly
            locked={[]}
            onOpen={setPath}
            onCreateFile={noop}
            onCreateFolder={noop}
            onRename={noop}
            onDelete={noop}
            onUpload={noop}
          />
        )}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 flex-wrap items-start gap-3 p-4 pb-3 lg:p-6 lg:pb-4">
          <Button variant="ghost" size="icon-sm" aria-label="Back to prompts" onClick={onBack}>
            <ArrowLeftIcon />
          </Button>
          <div className="min-w-0 flex-1 space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="m-0 font-mono text-lg font-semibold">{prompt?.name ?? "…"}</h1>
              {version && (
                <Badge variant="secondary" className="bg-muted font-normal text-muted-foreground">
                  v{version.number}
                </Badge>
              )}
              {version && <PromptFormatBadge format={version.format} />}
              {version?.format === PromptFormat.DYNAMIC ? (
                <span className="text-xs text-muted-foreground">Dynamic — rendered at runtime</span>
              ) : (
                version?.variables.map((v) => (
                  <Badge key={v} variant="outline" className="font-mono">
                    {variable(version.format, v)}
                  </Badge>
                ))
              )}
            </div>
            {version && (
              <p className="m-0 text-xs text-muted-foreground">
                {version.commitSha && (
                  <span className="font-mono" title={version.hash}>
                    {version.commitSha.slice(0, 7)} ·{" "}
                  </span>
                )}
                Deployed {when(version.createdAt)}
                {usage && (
                  <>
                    {" · "}
                    {Number(usage.requests).toLocaleString("en-US")} inference calls ·{" "}
                    {Number(usage.inputTokens).toLocaleString("en-US")} in /{" "}
                    {Number(usage.outputTokens).toLocaleString("en-US")} out tokens
                    {usage.costMicros !== undefined && ` · ${dollars(usage.costMicros)}`} ·{" "}
                    {Number(usage.averageLatencyMs).toLocaleString("en-US")} ms avg · last used{" "}
                    {when(usage.lastUsedAt)}
                  </>
                )}
              </p>
            )}
          </div>
          {error && (
            <p role="alert" className="m-0 basis-full text-sm text-destructive">
              {error}
            </p>
          )}
        </div>
        {file ? (
          <OpenFile file={file} editing={false} onChange={noop} />
        ) : (
          !error && <TildeLoader className="flex-1" />
        )}
      </div>
    </section>
  );
}
