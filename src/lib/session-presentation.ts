/** Plain display excerpts only. The transcript and indexed source remain untouched. */
export function sessionExcerpt(value: string): string {
  const entities: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };
  value = value.split("## My request:").pop() ?? value;
  if (/^\s*(?:<recommended_plugins>|<environment_context>|<skills_instructions>|# AGENTS\.md instructions)/.test(value)) return "";
  return value.replace(/&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (original, entity: string) => {
    if (!entity.startsWith("#")) return entities[entity.toLowerCase()] ?? original;
    const code = entity[1].toLowerCase() === "x" ? parseInt(entity.slice(2), 16) : Number(entity.slice(1));
    return code > 0 && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff) ? String.fromCodePoint(code) : original;
  }).replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}(?:#{1,6}\s+|>\s*|[-*+]\s+)/gm, "")
    .replace(/(?:\*\*|__|~~|`{1,3})/g, "")
    .replace(/(?:\|\s*:?-{3,}:?\s*)+\|?/g, " ")
    .replace(/\s+/g, " ").trim();
}
export function sessionDisplayTitle(title: string, fallback: string): string {
  const clean = sessionExcerpt(title);
  return /^(?:auto|untitled|new session)?$/i.test(clean) ? fallback : clean;
}
