import Link from "next/link";
import type { Route } from "next";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { cn } from "@/lib/utils";

/**
 * Renders markdown from the API. Raw HTML in the source is not rendered (react-markdown drops
 * it without a raw-HTML plugin), so a doc cannot inject markup.
 */
export function Markdown({
  children,
  linkBase = "/help",
}: {
  children: string;
  linkBase?: string;
}) {
  return (
    <div className="max-w-3xl min-w-0 text-sm leading-relaxed [overflow-wrap:anywhere]">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components(linkBase)}>
        {children}
      </ReactMarkdown>
    </div>
  );
}

/** `other.md` and `other.md#part` stay inside the help pages; anything external opens apart. */
export function resolveHref(
  href: string | undefined,
  linkBase: string,
): { href: string; external: boolean } {
  if (!href) return { href: "#", external: false };
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) return { href, external: true };
  const m = href.match(/^([a-z0-9_-]+)\.md(#.*)?$/);
  if (m) return { href: `${linkBase}/${m[1]}${m[2] ?? ""}`, external: false };
  return { href, external: false };
}

function components(linkBase: string): Components {
  return {
    h1: ({ children }) => (
      <h1 className="mt-2 mb-4 text-2xl font-semibold tracking-tight">{children}</h1>
    ),
    h2: ({ children }) => <h2 className="mt-8 mb-3 text-lg font-semibold">{children}</h2>,
    h3: ({ children }) => <h3 className="mt-6 mb-2 text-base font-semibold">{children}</h3>,
    p: ({ children }) => <p className="my-3">{children}</p>,
    ul: ({ children }) => <ul className="my-3 list-disc space-y-1 pl-6">{children}</ul>,
    ol: ({ children }) => <ol className="my-3 list-decimal space-y-1 pl-6">{children}</ol>,
    blockquote: ({ children }) => (
      <blockquote className="my-3 border-l-2 pl-4 text-muted-foreground">{children}</blockquote>
    ),
    hr: () => <hr className="my-6" />,
    a: ({ href, children }) => {
      const r = resolveHref(href, linkBase);
      const cls = "font-medium underline underline-offset-4 hover:text-muted-foreground";
      return r.external ? (
        <a href={r.href} target="_blank" rel="noopener noreferrer" className={cls}>
          {children}
        </a>
      ) : (
        <Link href={r.href as Route} className={cls}>
          {children}
        </Link>
      );
    },
    code: ({ className, children }) => (
      <code className={cn("rounded-sm bg-muted px-1 py-0.5 font-mono text-xs", className)}>
        {children}
      </code>
    ),
    pre: ({ children }) => (
      <pre className="my-4 overflow-x-auto rounded-lg bg-muted p-3 font-mono text-xs [&_code]:bg-transparent [&_code]:p-0">
        {children}
      </pre>
    ),
    table: ({ children }) => (
      <div className="my-4 overflow-x-auto rounded-lg border">
        {/* A table scrolls in its own box, so its words never break mid-word. */}
        <table className="w-full text-left text-xs [overflow-wrap:normal] [&_code]:whitespace-nowrap">
          {children}
        </table>
      </div>
    ),
    th: ({ children }) => <th className="border-b bg-muted px-3 py-2 font-medium">{children}</th>,
    td: ({ children }) => <td className="border-b px-3 py-2 align-top">{children}</td>,
  };
}
