// Adapted from trytilde/dispatch packages/ui. See ../LICENSE.dispatch.
"use client";
import { Children, useEffect, useRef, useState, type ReactNode } from "react";
import { Streamdown, type Components } from "streamdown";

export interface CodeBlockProps {
  children: string;
  language?: string;
  showLineNumbers?: boolean;
  wordWrap?: boolean;
}

export interface CitationLinkProps {
  children: ReactNode;
  href: string;
  label?: string;
}

export function CodeBlock({
  children,
  language = "",
  showLineNumbers = false,
  wordWrap = false,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timeoutRef.current), []);

  function copy(): void {
    void navigator.clipboard?.writeText(children);
    setCopied(true);
    clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setCopied(false), 2_000);
  }

  if (language === "diff" || language === "patch") {
    return <DiffBlock value={children} />;
  }

  const lines = children.replace(/\n$/, "").split("\n");
  return (
    <section
      className="not-prose my-2 w-full overflow-hidden rounded-card bg-surface shadow-hairline"
      data-word-wrap={wordWrap || undefined}
    >
      <header className="flex items-center justify-between border-b border-line px-3 py-1.5">
        <span className="text-[11.5px] text-ink-3">{language || "Code"}</span>
        <button
          aria-label={copied ? "Copied" : "Copy code"}
          onClick={copy}
          type="button"
          className={`flex h-6 items-center gap-1 rounded-[6px] px-1.5 text-[11.5px] font-medium
            transition-colors duration-100 hover:bg-hover
            ${copied ? "text-green" : "text-ink-3 hover:text-ink"}`}
        >
          {copied ? (
            <svg
              aria-hidden
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M20 6L9 17l-5-5" />
            </svg>
          ) : (
            <svg
              aria-hidden
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="9" y="9" width="12" height="12" rx="2.5" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
          )}
          {copied ? "Copied" : "Copy"}
        </button>
      </header>
      <pre
        className={`m-0 overflow-x-auto bg-inset px-3 py-2.5 font-mono text-[11.5px] leading-[1.7] text-ink-2
          ${wordWrap ? "whitespace-pre-wrap" : ""}`}
      >
        {lines.map((line, index) => (
          <div className="flex" key={`${index}-${line}`}>
            {showLineNumbers ? (
              <span
                aria-hidden="true"
                className="w-5 shrink-0 select-none text-right text-[10.5px] leading-[1.86] text-ink-3/60"
              >
                {index + 1}
              </span>
            ) : null}
            <span className={`whitespace-pre ${showLineNumbers ? "pl-2.5" : ""}`}>
              {line || " "}
            </span>
          </div>
        ))}
      </pre>
    </section>
  );
}

export function DiffBlock({ value }: { value: string }) {
  return (
    <section className="ob-code-block ob-code-block--diff">
      <header className="ob-code-block-header">
        <span>Diff</span>
      </header>
      <code className="ob-default-diff">
        {value
          .replace(/\n$/, "")
          .split("\n")
          .map((line, index) => {
            const tone = line.startsWith("+")
              ? "added"
              : line.startsWith("-")
                ? "removed"
                : "plain";
            return (
              <span data-tone={tone} key={`${index}-${line}`}>
                <i aria-hidden="true">{index + 1}</i>
                <span>{line || " "}</span>
              </span>
            );
          })}
      </code>
    </section>
  );
}

export function InlinePath({ value }: { value: string }) {
  const slash = Math.max(value.lastIndexOf("/"), value.lastIndexOf("\\"));
  const prefix = slash >= 0 ? value.slice(0, slash + 1) : "";
  const filename = slash >= 0 ? value.slice(slash + 1) : value;
  return (
    <code className="ob-markdown__inline-code" data-path="true">
      {prefix ? <span className="ob-markdown__inline-path-prefix">{prefix}</span> : null}
      <span className="ob-markdown__inline-path-filename">{filename}</span>
    </code>
  );
}

export function CitationLink({ children, href, label }: CitationLinkProps) {
  return (
    <a
      aria-label={label}
      className="ob-markdown__link ob-markdown__citation-btn md-citation-btn"
      href={href}
      rel="noreferrer"
      target="_blank"
    >
      {children}
    </a>
  );
}

const markdownComponents: Components = {
  a({ children, href = "" }) {
    const citation = /^\[?\d+\]?$/.test(plainText(children)) || href.startsWith("#citation-");
    return citation ? (
      <CitationLink href={href}>{children}</CitationLink>
    ) : (
      <a className="ob-markdown__link" href={href} rel="noreferrer" target="_blank">
        {children}
      </a>
    );
  },
  blockquote({ children }) {
    return <blockquote className="ob-markdown__blockquote">{children}</blockquote>;
  },
  code({ children, className }) {
    const value = plainText(children);
    const language = /language-([^ ]+)/.exec(className ?? "")?.[1] ?? "";
    if (className || value.includes("\n"))
      return <CodeBlock language={language}>{value}</CodeBlock>;
    if (looksLikePath(value)) return <InlinePath value={value} />;
    return <code className="ob-markdown__inline-code">{children}</code>;
  },
  del({ children }) {
    return <del className="ob-markdown__del">{children}</del>;
  },
  h1: ({ children }) => <h1 className="ob-markdown__heading">{children}</h1>,
  h2: ({ children }) => <h2 className="ob-markdown__heading">{children}</h2>,
  h3: ({ children }) => <h3 className="ob-markdown__heading">{children}</h3>,
  h4: ({ children }) => <h4 className="ob-markdown__heading">{children}</h4>,
  h5: ({ children }) => <h5 className="ob-markdown__heading">{children}</h5>,
  h6: ({ children }) => <h6 className="ob-markdown__heading">{children}</h6>,
  hr: () => <hr className="ob-markdown__hr" />,
  img({ alt = "", src }) {
    return src ? (
      <img alt={alt} className="ob-markdown__image" loading="lazy" src={src} />
    ) : (
      <span className="ob-markdown__broken-image">
        <span aria-hidden="true">▧</span>
        <span className="ob-markdown__broken-image-label">{alt || "Image unavailable"}</span>
      </span>
    );
  },
  input({ type, ...props }) {
    return (
      <input
        className={type === "checkbox" ? "ob-markdown__task-marker" : undefined}
        type={type}
        {...(props as React.InputHTMLAttributes<HTMLInputElement>)}
      />
    );
  },
  li({ children, className }) {
    return <li className={`ob-markdown__list-item ${className ?? ""}`}>{children}</li>;
  },
  ol: ({ children }) => <ol className="ob-markdown__list">{children}</ol>,
  p: ({ children }) => <p className="ob-markdown__paragraph">{children}</p>,
  pre: ({ children }) => <>{children}</>,
  table: ({ children }) => <table className="ob-markdown__table">{children}</table>,
  tbody: ({ children }) => <tbody>{children}</tbody>,
  td: ({ children }) => (
    <td className="ob-markdown__td">
      <span className="ob-markdown__table-cell-content">{children}</span>
    </td>
  ),
  th: ({ children }) => (
    <th className="ob-markdown__th">
      <span className="ob-markdown__table-cell-content">{children}</span>
    </th>
  ),
  thead: ({ children }) => <thead className="ob-markdown__thead">{children}</thead>,
  tr: ({ children }) => <tr className="ob-markdown__tr">{children}</tr>,
  ul: ({ children }) => <ul className="ob-markdown__list">{children}</ul>,
};

export function MarkdownText({
  text,
  allowImages = true,
}: {
  text: string;
  allowImages?: boolean;
}) {
  if (!text) return null;
  return (
    <div className="markdown ob-markdown">
      <Streamdown
        components={
          allowImages
            ? markdownComponents
            : {
                ...markdownComponents,
                img: ({ alt }) => <span>[Image{alt ? `: ${alt}` : ""}]</span>,
              }
        }
        controls={false}
      >
        {text}
      </Streamdown>
    </div>
  );
}

function plainText(value: ReactNode): string {
  return Children.toArray(value)
    .map((child) => (typeof child === "string" || typeof child === "number" ? `${child}` : ""))
    .join("");
}

function looksLikePath(value: string): boolean {
  return (
    /^(?:\.{0,2}[/\\]|~[/\\]|[/\\]|[A-Za-z]:[/\\])/.test(value) ||
    (/[/\\]/.test(value) && /\.[A-Za-z0-9]{1,8}$/.test(value))
  );
}
