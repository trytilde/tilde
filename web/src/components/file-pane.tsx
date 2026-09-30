import { Suspense, lazy, useEffect, useState } from "react";
import { DownloadIcon } from "lucide-react";
import { splitFrontMatter } from "./skill-common";
import { MarkdownEditor } from "./markdown-editor";
import { buttonVariants } from "./ui/button";

/** A file as the skill and prompt views open it; stored binaries download from `downloadUrl`. */
export type PaneFile = {
  path: string;
  content: string;
  binary: boolean;
  /** Bytes chosen in the browser and not saved yet. */
  data?: Uint8Array;
  mediaType: string;
  sizeBytes: bigint;
  downloadUrl: string;
};
const isMarkdown = (path: string) => /\.(md|markdown)$/i.test(path);

/** The open file under a breadcrumb of its path, filling the rest of the view. */
export function OpenFile({
  file,
  editing,
  onChange,
  hideFrontMatter = false,
}: {
  file: PaneFile;
  editing: boolean;
  onChange: (content: string) => void;
  /** Markdown front matter is shown and edited elsewhere (SKILL.md's name and description). */
  hideFrontMatter?: boolean;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col border-t">
      <div className="flex h-9 shrink-0 items-center gap-1.5 border-b bg-muted/30 px-4 text-[13px] lg:px-6">
        <span className="text-muted-foreground">
          {file.path.split("/").slice(0, -1).join(" › ")}
        </span>
        {file.path.includes("/") && <span className="text-muted-foreground">›</span>}
        <span className="font-medium">{file.path.split("/").pop()}</span>
      </div>
      <FilePane
        file={file}
        editing={editing}
        onChange={onChange}
        hideFrontMatter={hideFrontMatter}
      />
    </div>
  );
}

function size(bytes: bigint | number) {
  const n = Number(bytes);
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

const CodeEditor = lazy(() => import("./code-editor"));
const isImage = (file: PaneFile) =>
  file.mediaType.startsWith("image/") || /\.(png|jpe?g|gif|webp|svg|ico|bmp)$/i.test(file.path);

/**
 * The open file, as VS Code shows it: markdown in the rich editor, other text in Monaco, images as a preview,
 * and anything else as a card to download.
 */
function FilePane({
  file,
  editing,
  onChange,
  hideFrontMatter = false,
}: {
  file: PaneFile;
  editing: boolean;
  onChange: (content: string) => void;
  /** Markdown front matter is shown and edited elsewhere (SKILL.md's name and description). */
  hideFrontMatter?: boolean;
}) {
  const [objectUrl, setObjectUrl] = useState<string>();
  useEffect(() => {
    if (!file.data) return setObjectUrl(undefined);
    const url = URL.createObjectURL(new Blob([file.data.slice()], { type: file.mediaType }));
    setObjectUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [file.data, file.mediaType]);
  if (file.binary) {
    const src = objectUrl ?? file.downloadUrl;
    return isImage(file) && src ? (
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-[repeating-conic-gradient(var(--muted)_0_25%,transparent_0_50%)] bg-[length:16px_16px] p-6">
        <img src={src} alt={file.path} className="max-h-full max-w-full object-contain" />
      </div>
    ) : (
      <div className="p-4 lg:p-6">
        <div className="flex items-center gap-4 rounded-lg border bg-muted/30 p-4 text-sm">
          <div className="min-w-0 flex-1">
            <p className="m-0 font-mono text-xs font-medium">{file.path}</p>
            <p className="m-0 text-xs text-muted-foreground">
              {file.mediaType || "Binary file"} · {size(file.sizeBytes)}
            </p>
          </div>
          {file.downloadUrl && !file.data ? (
            <a
              href={file.downloadUrl}
              download={file.path.split("/").pop()}
              className={buttonVariants({ variant: "outline", size: "sm" })}
            >
              <DownloadIcon />
              Download
            </a>
          ) : (
            <span className="text-xs text-muted-foreground">Saved with the skill</span>
          )}
        </div>
      </div>
    );
  }
  // Prompts and other skill files show their text whole, front matter included.
  const split = hideFrontMatter ? splitFrontMatter(file.content) : { head: "", body: file.content };
  if (isMarkdown(file.path))
    return (
      <div className="min-h-0 flex-1 overflow-y-auto p-4 lg:p-6">
        <MarkdownEditor
          key={file.path}
          label={`Contents of ${file.path}`}
          value={split.body}
          readOnly={!editing}
          onChange={(body) => onChange(split.head + body)}
          minHeight="min-h-[50vh]"
        />
      </div>
    );
  return (
    <div className="min-h-0 flex-1" aria-label={`Contents of ${file.path}`}>
      <Suspense
        fallback={
          <pre className="m-0 h-full overflow-auto p-4 font-mono text-xs leading-5 whitespace-pre-wrap">
            {file.content}
          </pre>
        }
      >
        <CodeEditor path={file.path} value={file.content} readOnly={!editing} onChange={onChange} />
      </Suspense>
    </div>
  );
}
