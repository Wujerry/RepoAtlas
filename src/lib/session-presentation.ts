/** Strip known client envelopes for display only. Never alter the indexed/original text. */
export function sessionDisplayText(value: string): string {
  value = value.split("## My request:").pop() ?? value;
  const envelope = /^\s*<(external_codex_apps_open_page|subagent_notification|environment_context|skills_instructions|recommended_plugins|permissions instructions|collaboration_mode|turn_aborted)\b[^>]*>(?:[\s\S]*?<\/\1>\s*|[\s\S]*$)/i;
  while (envelope.test(value)) value = value.replace(envelope, "");
  if (/^\s*# AGENTS\.md instructions/.test(value)) return "";
  value = value.replace(/\s*<oai-mem-citation>[\s\S]*?<\/oai-mem-citation>\s*$/i, "");
  // Attachment transport markup is not the attachment itself. Do not expose
  // base64 payloads or automatically load local/remote paths in the reader.
  value = value.replace(/<image\b[^>]*>[\s\S]*?<\/image>/gi, "\n![image attachment]()\n");
  value = value.replace(/<image\s+(?:name|path)=[^>\n]*[\s\S]*$/gi, "");
  return value;
}

/** Plain display excerpts only. The transcript and indexed source remain untouched. */
export function sessionExcerpt(value: string): string {
  const entities: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };
  value = sessionDisplayText(value);
  return value.replace(/&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (original, entity: string) => {
    if (!entity.startsWith("#")) return entities[entity.toLowerCase()] ?? original;
    const code = entity[1].toLowerCase() === "x" ? parseInt(entity.slice(2), 16) : Number(entity.slice(1));
    return code > 0 && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff) ? String.fromCodePoint(code) : original;
  }).replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*$/g, "$1")
    .replace(/^\s{0,3}(?:#{1,6}\s+|>\s*|[-*+]\s+)/gm, "")
    .replace(/(?:\*\*|__|~~|`{1,3})/g, "")
    .replace(/(?:\|\s*:?-{3,}:?\s*)+\|?/g, " ")
    .replace(/\s+/g, " ").trim();
}
export function sessionDisplayTitle(title: string, fallback: string): string {
  let clean = sessionExcerpt(title);
  // A pasted quotation followed by a request should be named after that request,
  // using only the user's own words, never a generated summary.
  const closingQuote = ({ '“': '”', '‘': '’', '"': '"' } as Record<string, string>)[clean[0]];
  if (closingQuote) {
    const end = clean.lastIndexOf(closingQuote);
    const request = end > 0 ? clean.slice(end + 1).replace(/^[\s，,。:：]+/, "").trim() : "";
    if (request.length >= 4) clean = request;
  }
  return /^(?:auto|untitled|new session)?$/i.test(clean) ? fallback : clean;
}
