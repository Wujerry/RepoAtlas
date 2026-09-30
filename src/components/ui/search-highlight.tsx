/** Render source text literally; search terms never become HTML. */
export function SearchHighlight({ text, query }: { text: string; query: string }) {
  const terms = query.trim().split(/\s+/).filter(Boolean).map(term => term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  if (!terms.length) return <>{text}</>;
  return <>{text.split(new RegExp(`(${terms.join("|")})`, "gi")).map((part, index) => index % 2 ? <mark key={index}>{part}</mark> : part)}</>;
}

/** Keep the match visible even when it occurs late in a long message. */
export function searchExcerpt(text: string, query: string, limit = 220, before = 60) {
  const lower = text.toLowerCase();
  const positions = query.trim().toLowerCase().split(/\s+/).filter(Boolean).map(term => lower.indexOf(term)).filter(index => index >= 0);
  const start = Math.max(0, (positions.length ? Math.min(...positions) : 0) - before);
  const end = Math.min(text.length, start + limit);
  return `${start ? "…" : ""}${text.slice(start, end)}${end < text.length ? "…" : ""}`;
}
