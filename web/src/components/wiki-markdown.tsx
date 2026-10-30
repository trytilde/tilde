import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Link } from "@tanstack/react-router";

/** A URL with a scheme (`https:`, `mailto:`) or a protocol-relative one: outside the wiki. */
const absolute = (href: string) => /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("//");
/** The wiki path a link names: from the root, with a leading `./` or `/` and `.md` dropped. */
const pagePath = (href: string) =>
  href
    .split("#")[0]
    .replace(/^(\.\/|\/)+/, "")
    .replace(/\/+$/, "")
    .replace(/\.md$/i, "");
/** Whether an image would load from this app's own origin. */
function sameOrigin(src: string) {
  try {
    return new URL(src, window.location.href).origin === window.location.origin;
  } catch {
    return false;
  }
}

/**
 * A wiki page's Markdown with GitHub's extensions (tables, task lists, strikethrough). Raw HTML
 * in the page is not rendered and link targets pass react-markdown's safe-URL check. A link
 * without a scheme is another page of the wiki by its path from the root
 * (`[Deploys](/runbooks/deploys)`) and opens here; absolute links open outside the app. Images from other origins are shown as links
 * and never fetched, so viewing a page cannot reveal who read it.
 */
export function WikiMarkdown({
  content,
  label,
  wikiId,
}: {
  content: string;
  label: string;
  wikiId: string;
}) {
  return (
    <article aria-label={label} className="wiki-prose">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          a: ({ children, href = "" }) =>
            href.startsWith("#") ? (
              <a href={href}>{children}</a>
            ) : absolute(href) ? (
              <a href={href} target="_blank" rel="noopener noreferrer">
                {children}
              </a>
            ) : (
              <Link to="/wikis/$wikiId/pages" params={{ wikiId }} search={{ page: pagePath(href) }}>
                {children}
              </Link>
            ),
          img: ({ src = "", alt }) =>
            typeof src === "string" && sameOrigin(src) ? (
              <img src={src} alt={alt ?? ""} />
            ) : (
              <a href={String(src)} target="_blank" rel="noopener noreferrer">
                [Image{alt ? `: ${alt}` : ""}]
              </a>
            ),
        }}
      >
        {content}
      </ReactMarkdown>
    </article>
  );
}
