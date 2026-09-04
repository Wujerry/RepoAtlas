import { openUrl } from "@tauri-apps/plugin-opener";
import DOMPurify from "dompurify";
import { Children, isValidElement, memo, useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import rehypeSlug from "rehype-slug";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import remarkGithubBlockquoteAlert from "remark-github-blockquote-alert";
import remarkMath from "remark-math";
import { api } from "../lib/api";
import { CodePreview, useDarkTheme } from "./CodePreview";

const safeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), "details", "summary", "kbd", "mark", "sub", "sup"],
  attributes: {
    ...defaultSchema.attributes,
    "*": [...(defaultSchema.attributes?.["*"] ?? []), "className", "id"],
    input: [...(defaultSchema.attributes?.input ?? []), ["type", "checkbox"], "checked", "disabled"],
  },
  protocols: {
    ...defaultSchema.protocols,
    href: ["http", "https", "mailto"],
    src: ["http", "https"],
  },
};

function resolveProjectPath(documentPath: string, target: string) {
  const cleanTarget = target.split(/[?#]/, 1)[0]?.replace(/\\/g, "/") ?? "";
  if (!cleanTarget || cleanTarget.startsWith("/") || /^[a-z][a-z0-9+.-]*:/i.test(cleanTarget)) return null;
  const base = documentPath.replace(/\\/g, "/").split("/").slice(0, -1);
  for (const part of cleanTarget.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      if (base.length === 0) return null;
      base.pop();
    } else {
      base.push(part);
    }
  }
  return base.join("/");
}

function MermaidDiagram({ source }: { source: string }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const id = useId().replace(/:/g, "-");
  const [visible, setVisible] = useState(false);
  const [svg, setSvg] = useState<string>();
  const [error, setError] = useState<string>();
  const dark = useDarkTheme();

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const observer = new IntersectionObserver(([entry]) => entry?.isIntersecting && setVisible(true), { rootMargin: "240px" });
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return;
    let active = true;
    void import("mermaid").then(async ({ default: mermaid }) => {
      mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: dark ? "dark" : "neutral" });
      const result = await mermaid.render(`repoatlas-mermaid-${id}`, source);
      if (active) setSvg(DOMPurify.sanitize(result.svg, { USE_PROFILES: { svg: true, svgFilters: true } }));
    }).catch((reason) => active && setError(String(reason)));
    return () => { active = false; };
  }, [dark, id, source, visible]);

  return <div ref={hostRef} className="markdown-mermaid">
    {svg ? <div dangerouslySetInnerHTML={{ __html: svg }} /> : error ? <pre><code>{source}</code></pre> : <span className="muted-copy">Mermaid…</span>}
  </div>;
}

function safeImageDimension(value: string | number | undefined) {
  const parsed = typeof value === "number" ? value : Number.parseInt(value ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 && parsed <= 100_000 ? Math.round(parsed) : undefined;
}

function LocalMarkdownImage({ projectId, documentPath, source, alt, width, height }: {
  projectId: string;
  documentPath: string;
  source: string;
  alt: string;
  width?: string | number;
  height?: string | number;
}) {
  const [url, setUrl] = useState<string>();
  const [visible, setVisible] = useState(false);
  const hostRef = useRef<HTMLSpanElement>(null);
  const path = resolveProjectPath(documentPath, source);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const observer = new IntersectionObserver(([entry]) => entry?.isIntersecting && setVisible(true), { rootMargin: "240px" });
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible || !path) return;
    let active = true;
    let objectUrl: string | undefined;
    void api.readProjectImage(projectId, path).then(async (data) => {
      if (!active) return;
      objectUrl = URL.createObjectURL(new Blob([data]));
      const decodedImage = new Image();
      decodedImage.src = objectUrl;
      if (typeof decodedImage.decode === "function") {
        await decodedImage.decode().catch(() => undefined);
      }
      if (!active) return;
      setUrl(objectUrl);
    }).catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path, projectId, visible]);

  const safeWidth = safeImageDimension(width);
  const safeHeight = safeImageDimension(height);
  return <span ref={hostRef} className="readme-local-image">{url ? <img src={url} alt={alt} width={safeWidth} height={safeHeight} decoding="sync" draggable={false} /> : <span className="readme-image-placeholder">{alt || source}</span>}</span>;
}

function MarkdownCodeBlock({ children }: { children?: ReactNode }) {
  const child = Children.toArray(children)[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(child)) return <pre>{children}</pre>;
  const language = /language-([\w-]+)/.exec(child.props.className ?? "")?.[1];
  const source = String(child.props.children ?? "").replace(/\n$/, "");
  if (language === "mermaid") return <MermaidDiagram source={source} />;
  return <CodePreview code={source} language={language} compact />;
}

type MarkdownDocumentProps = {
  content: string;
  projectId?: string;
  documentPath?: string;
  onOpenPath?: (path: string) => void;
};

const MarkdownDocumentContent = memo(function MarkdownDocumentContent({ content, projectId, documentPath = "README.md", onOpenPath }: MarkdownDocumentProps) {
  return <article className="readme-document"><ReactMarkdown
    remarkPlugins={[remarkGfm, remarkFrontmatter, remarkMath, remarkGithubBlockquoteAlert]}
    rehypePlugins={[rehypeRaw, [rehypeSanitize, safeSchema], rehypeKatex, rehypeSlug]}
    components={{
      a: ({ href, children }) => {
        const external = Boolean(href && /^https?:\/\//i.test(href));
        const relative = href ? resolveProjectPath(documentPath, href) : null;
        const blocked = Boolean(href && !external && !relative && !href.startsWith("#") && !href.startsWith("mailto:"));
        return <a href={blocked ? undefined : href} onClick={(event) => {
          if (external) { event.preventDefault(); void openUrl(href!); }
          else if (relative) { event.preventDefault(); onOpenPath?.(relative); }
          else if (blocked) event.preventDefault();
        }}>{children}</a>;
      },
      img: ({ src, alt, width, height }) => {
        if (!src) return <span className="readme-image-placeholder">{alt || "Image"}</span>;
        if (/^https?:\/\//i.test(src)) return <button type="button" className="readme-image-placeholder" onClick={() => void openUrl(src)}>{alt || src}</button>;
        if (!projectId) return <span className="readme-image-placeholder">{alt || "Image"}</span>;
        return <LocalMarkdownImage projectId={projectId} documentPath={documentPath} source={src} alt={alt ?? ""} width={width} height={height} />;
      },
      pre: MarkdownCodeBlock,
      code: ({ className, children }) => <code className={className}>{children as ReactNode}</code>,
    }}
  >{content}</ReactMarkdown></article>;
}, (previous, next) => (
  previous.content === next.content
  && previous.projectId === next.projectId
  && previous.documentPath === next.documentPath
  && previous.onOpenPath === next.onOpenPath
));

export function MarkdownDocument(props: MarkdownDocumentProps) {
  const onOpenPathRef = useRef(props.onOpenPath);
  onOpenPathRef.current = props.onOpenPath;
  const onOpenPath = useCallback((path: string) => onOpenPathRef.current?.(path), []);
  return <MarkdownDocumentContent {...props} onOpenPath={props.onOpenPath ? onOpenPath : undefined} />;
}
