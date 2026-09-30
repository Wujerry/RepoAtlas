import { CaretDown, CaretUp, Code, Copy, Image } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { memo, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { markdownNodeText, rehypeSessionHighlight, sessionMarkdownWindow } from "../../lib/session-markdown";
import type { MessageKey } from "../../i18n";
import { sessionDisplayText } from "../../lib/session-presentation";
import { Button } from "./button";
import { SearchHighlight } from "./search-highlight";
import { SessionCodeBlock } from "./session-code-block";

type T = (key: MessageKey) => string;
type Plugins = NonNullable<Parameters<typeof ReactMarkdown>[0]["rehypePlugins"]>;
const schema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), "details", "summary", "kbd", "mark", "sub", "sup"],
  attributes: { ...defaultSchema.attributes, code: [["className", /^language-./, "math-inline", "math-display"]] },
};
const remarkPlugins = [remarkGfm];
const messagePlugins = [remarkGfm, [remarkMath, { singleDollarTextMath: false }]] as Plugins;
const PAGE_SIZE = 12_000;

const RenderedMessage = memo(function RenderedMessage({ content, query, t }: { content: string; query: string; t: T }) {
  const [linkError, setLinkError] = useState(false);
  const plugins = useMemo(() => [rehypeRaw, [rehypeSanitize, schema], [rehypeKatex, { trust: false, strict: "ignore" }], [rehypeSessionHighlight, { query }]] as Plugins, [query]);
  const components = useMemo<Components>(() => ({
    a: ({ href, children }) => <a href={href && /^https?:\/\//i.test(href) ? href : undefined} onClick={event => {
      event.preventDefault();
      if (href && /^https?:\/\//i.test(href)) { setLinkError(false); void openUrl(href).catch(() => setLinkError(true)); }
    }}>{children}</a>,
    img: ({ alt }) => <span className="session-attachment"><Image aria-hidden size={15} />{alt || t("sessionAttachment")}</span>,
    pre: ({ node }) => {
      const code = node?.children.find(child => child.type === "element" && child.tagName === "code");
      if (!code || code.type !== "element") return <pre>{node ? markdownNodeText(node) : ""}</pre>;
      const language = /language-([\w+-]+)/.exec(String(code.properties.className ?? ""))?.[1];
      const source = markdownNodeText(code).replace(/\n$/, "");
      return <SessionCodeBlock code={source} language={language} query={query} t={t} />;
    },
    table: ({ children }) => <div className="session-table-scroll" tabIndex={0} role="region" aria-label={t("sessionTable")}><table>{children}</table></div>,
  }), [query, t]);
  return <><div className="session-markdown"><ReactMarkdown remarkPlugins={messagePlugins} rehypePlugins={plugins} components={components}>{content}</ReactMarkdown></div>
    {linkError && <p role="alert">{t("sessionLinkFailed")}</p>}</>;
});

/** All readers share bounded Markdown; original copy never uses a display excerpt. */
export const SessionContent = memo(function SessionContent({ content, query = "", t, limit = 4000, copy = false }: {
  content: string; query?: string; t: T; limit?: number; copy?: boolean;
}) {
  const [state, setState] = useState({ content, query, raw: false, expanded: false, offset: undefined as number | undefined });
  // Reused preview slots must not inherit the previous message's raw mode/page.
  if (state.content !== content || state.query !== query) setState({ content, query, raw: false, expanded: false, offset: undefined });
  const { raw, expanded, offset } = state;
  const [copyState, setCopyState] = useState<"idle" | "pending" | "copied" | "error">("idle");
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => { setCopyState("idle"); }, [content]);
  useEffect(() => { if (offset !== undefined) host.current?.scrollIntoView({ block: "start" }); }, [offset]);
  const display = useMemo(() => sessionDisplayText(content), [content]);
  const source = raw ? content : display;
  const budget = expanded ? PAGE_SIZE : Math.min(limit, PAGE_SIZE);
  const window = useMemo(() => sessionMarkdownWindow(source, query, budget, offset), [source, query, budget, offset]);
  const technical = !source.trim() && !raw;
  const clipped = window.before || window.after;
  return <div ref={host} className={`session-content${technical ? " is-technical" : ""}`}>
    {technical ? <p className="session-content-folded">{t("sessionTechnical")}</p>
      : <>{window.before && <p className="session-continuation">{t("sessionEarlier")}</p>}
        {raw ? <pre className="session-source"><SearchHighlight text={source.slice(window.start, window.end)} query={query} /></pre>
          : <RenderedMessage content={window.text} query={query} t={t} />}
        {window.after && <p className="session-continuation">{t("sessionContinued")}</p>}</>}
    <div className="session-content-controls">
      {source.length > limit && <Button variant="quiet" aria-expanded={expanded} onClick={() => setState(value => ({ ...value, expanded: !value.expanded, offset: undefined }))}>
        {expanded ? <CaretUp aria-hidden /> : <CaretDown aria-hidden />}{t(expanded ? "qsCollapse" : "qsExpand")}</Button>}
      {expanded && clipped && <div className="session-page-controls" role="group" aria-label={t("sessionMessageParts")}>
        <Button variant="quiet" disabled={!window.before} onClick={() => setState(value => ({ ...value, offset: Math.max(0, window.start - PAGE_SIZE) }))}>{t("sessionPreviousPart")}</Button>
        <Button variant="quiet" disabled={!window.after} onClick={() => setState(value => ({ ...value, offset: window.end }))}>{t("sessionNextPart")}</Button>
      </div>}
      <span className="session-content-secondary"><Button variant="quiet" onClick={() => setState(value => ({ ...value, raw: !value.raw, expanded: false, offset: undefined }))} aria-pressed={raw}><Code aria-hidden />{t(raw ? "sessionRead" : "sessionRaw")}</Button>
        {copy && <Button variant="quiet" loading={copyState === "pending"} onClick={() => {
          setCopyState("pending"); void navigator.clipboard.writeText(content).then(() => setCopyState("copied")).catch(() => setCopyState("error"));
        }}><Copy aria-hidden />{t(copyState === "copied" ? "ahCopied" : "qsCopyMessage")}</Button>}</span>
    </div>{copyState === "error" && <p role="alert">{t("fpCopyFailed")}</p>}
  </div>;
});

// Result rows are buttons. Render only phrasing content: no nested links, controls,
// headings or full code/table layout; preserve emphasis and inline code instead.
function InlineBlock({ children }: { children?: ReactNode }) { return <span>{children}{" "}</span>; }
const snippetComponents: Components = {
  p: InlineBlock, h1: InlineBlock, h2: InlineBlock, h3: InlineBlock, h4: InlineBlock, h5: InlineBlock, h6: InlineBlock,
  ul: InlineBlock, ol: InlineBlock, li: InlineBlock, blockquote: InlineBlock, pre: InlineBlock,
  table: InlineBlock, thead: InlineBlock, tbody: InlineBlock, tr: InlineBlock, td: InlineBlock, th: InlineBlock,
  a: ({ children }) => <span>{children}</span>, img: ({ alt }) => <span>{alt}</span>, input: () => null,
  br: () => <span> </span>, hr: () => <span> </span>,
};
export const SessionSnippet = memo(function SessionSnippet({ content, query = "", fallback = "" }: { content: string; query?: string; fallback?: string }) {
  const text = useMemo(() => sessionMarkdownWindow(sessionDisplayText(content), query, 600).text, [content, query]);
  const plugins = useMemo(() => [[rehypeSessionHighlight, { query, skipCode: false }]] as Plugins, [query]);
  return <span className="session-snippet">{text.trim() ? <ReactMarkdown remarkPlugins={remarkPlugins} rehypePlugins={plugins} components={snippetComponents}>{text}</ReactMarkdown> : fallback}</span>;
});
