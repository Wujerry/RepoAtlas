import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ThemedToken } from "shiki/core";
import { highlightCode } from "../lib/shiki-client";

function resolveDarkTheme() {
  const theme = document.documentElement.dataset.theme;
  if (theme === "dark") return true;
  if (theme === "light") return false;
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

export function useDarkTheme() {
  const [dark, setDark] = useState(resolveDarkTheme);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const update = () => setDark(resolveDarkTheme());
    const themeObserver = new MutationObserver(update);
    media.addEventListener("change", update);
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    update();
    return () => {
      media.removeEventListener("change", update);
      themeObserver.disconnect();
    };
  }, []);
  return dark;
}

function tokenStyle(token: ThemedToken) {
  const style: React.CSSProperties = {};
  if (token.color) style.color = token.color;
  if (token.fontStyle && token.fontStyle & 1) style.fontStyle = "italic";
  if (token.fontStyle && token.fontStyle & 2) style.fontWeight = 600;
  if (token.fontStyle && token.fontStyle & 4) style.textDecoration = "underline";
  return style;
}

export function CodePreview({ code, language, compact = false }: { code: string; language?: string | null; compact?: boolean }) {
  const dark = useDarkTheme();
  const [tokens, setTokens] = useState<ThemedToken[][]>();
  const plainLines = useMemo(() => code.split("\n"), [code]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    setTokens(undefined);
    if (code.length > 512 * 1024) {
      setTokens([]);
      return () => { active = false; };
    }
    void highlightCode(code, language ?? "text", dark, controller.signal).then((lines) => {
      if (active) setTokens(lines);
    }).catch(() => {
      if (active) setTokens([]);
    });
    return () => {
      active = false;
      controller.abort();
    };
  }, [code, language, dark]);

  const lines = tokens?.length ? tokens : plainLines.map((line) => [{ content: line, offset: 0 }]);
  const virtualizer = useVirtualizer({
    count: lines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => compact ? 22 : 24,
    overscan: 12,
    initialRect: { width: 900, height: compact ? 420 : 700 },
  });

  return <div ref={scrollRef} className={`code-preview${compact ? " code-preview-compact" : ""}`} style={compact ? { height: Math.min(420, Math.max(44, lines.length * 22 + 2)) } : undefined}>
    <div className="code-preview-inner" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((row) => <div className="code-preview-line" key={row.key} style={{ transform: `translateY(${row.start}px)` }}>
        <span className="code-preview-number" aria-hidden="true">{row.index + 1}</span>
        <code>{lines[row.index]?.map((token, index) => <span key={`${index}-${token.offset}`} style={tokenStyle(token as ThemedToken)}>{token.content}</span>)}</code>
      </div>)}
    </div>
  </div>;
}
