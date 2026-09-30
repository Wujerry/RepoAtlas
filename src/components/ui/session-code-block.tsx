import { Check, Copy } from "@phosphor-icons/react";
import { memo, useEffect, useRef, useState } from "react";
import type { ThemedToken } from "shiki/core";
import type { MessageKey } from "../../i18n";
import { highlightCode } from "../../lib/shiki-client";
import { useDarkTheme } from "../CodePreview";
import { Button } from "./button";
import { SearchHighlight } from "./search-highlight";

const aliases: Record<string, string> = { ts: "typescript", js: "javascript", py: "python", rs: "rust", sh: "bash", shell: "bash", zsh: "bash", yml: "yaml", md: "markdown", ps1: "powershell", pwsh: "powershell", cs: "csharp", cxx: "cpp", dockerfile: "docker" };

/** Code stays selectable immediately; only visible blocks request worker highlighting. */
export const SessionCodeBlock = memo(function SessionCodeBlock({ code, language = "", query, t }: {
  code: string; language?: string; query: string; t: (key: MessageKey) => string;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [result, setResult] = useState<{ code: string; dark: boolean; lines: ThemedToken[][] }>();
  const [copyState, setCopyState] = useState<"idle" | "pending" | "copied" | "error">("idle");
  const [wrap, setWrap] = useState(false);
  const dark = useDarkTheme();
  const normalized = aliases[language.toLowerCase()] ?? language.toLowerCase();
  useEffect(() => {
    if (!host.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: "120px" });
    observer.observe(host.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!visible || !normalized || code.length > 12_000 || query.trim()) return;
    const controller = new AbortController();
    void highlightCode(code, normalized, dark, controller.signal).then(lines => {
      if (!controller.signal.aborted) setResult({ code, dark, lines });
    }).catch(() => { /* Plain code is the readable fallback. */ });
    return () => controller.abort();
  }, [code, normalized, dark, visible, query]);
  const lines = result?.code === code && result.dark === dark ? result.lines : undefined;
  return <div ref={host} className="session-code-block">
    <div className="session-code-toolbar"><span>{language || t("sessionCode")}</span><div>
      <Button variant="quiet" aria-pressed={wrap} onClick={() => setWrap(value => !value)}>{t("sessionWrap")}</Button>
      <Button variant="quiet" loading={copyState === "pending"} aria-label={t("sessionCopyCode")} onClick={() => {
        setCopyState("pending");
        void navigator.clipboard.writeText(code).then(() => setCopyState("copied")).catch(() => setCopyState("error"));
      }}>{copyState === "copied" ? <Check aria-hidden /> : <Copy aria-hidden />}{t(copyState === "copied" ? "ahCopied" : "sessionCopyCode")}</Button>
    </div></div>
    <pre tabIndex={0} aria-label={language || t("sessionCode")} className={wrap ? "is-wrapped" : undefined}><code>{query.trim() || !lines?.length
      ? <SearchHighlight text={code} query={query} />
      : lines.map((line, i) => <span key={i}>{i > 0 && "\n"}{line.map((token, j) => <span key={j} style={{ color: token.color, fontStyle: (token.fontStyle ?? 0) & 1 ? "italic" : undefined, fontWeight: (token.fontStyle ?? 0) & 2 ? 600 : undefined }}>{token.content}</span>)}</span>)}</code></pre>
    {copyState === "error" && <p role="alert">{t("fpCopyFailed")}</p>}
  </div>;
});
