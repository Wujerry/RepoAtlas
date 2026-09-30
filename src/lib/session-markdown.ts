// Highlight text nodes after Markdown parsing so emphasis, tables and lists keep
// their structure. Search strings are literal data, never HTML or a raw regex.
interface Node { type: string; value?: string; tagName?: string; properties?: Record<string, unknown>; children?: Node[] }
export function markdownNodeText(node: Node): string {
  return node.value ?? node.children?.map(markdownNodeText).join("") ?? "";
}

/** Slice around a match without turning a fenced code fragment into prose.
 * Offsets always refer to the untouched source, including at page boundaries. */
export function sessionMarkdownWindow(source: string, query: string, budget: number, offset?: number) {
  budget = Math.max(80, Math.min(12_000, budget));
  if (source.length <= budget && offset === undefined) return { start: 0, end: source.length, text: source, before: false, after: false };
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const lower = source.toLowerCase();
  const matches = terms.map(term => lower.indexOf(term)).filter(index => index >= 0);
  let start = offset ?? Math.max(0, (matches.length ? Math.min(...matches) : 0) - Math.min(160, Math.floor(budget / 3)));
  start = Math.max(0, Math.min(start, Math.max(0, source.length - 1)));
  if (offset === undefined) {
    const lineStart = source.lastIndexOf("\n", start - 1) + 1;
    if (start - lineStart < Math.min(160, budget / 3)) start = lineStart;
  }
  let end = Math.min(source.length, start + budget);
  const lineEnd = source.lastIndexOf("\n", end);
  if (end < source.length && lineEnd > start + budget * .8) end = lineEnd + 1;
  // Keep Unicode pairs intact when an unusually long line must be split.
  if (end < source.length && /[\uD800-\uDBFF]/.test(source[end - 1])) end--;
  if (start > 0 && /[\uDC00-\uDFFF]/.test(source[start])) start--;
  let fence: { marker: string; info: string } | undefined;
  let opening = "";
  let reachedWindow = false;
  const fences = /^ {0,3}(`{3,}|~{3,})([^\n]*)$/gm;
  for (const match of source.slice(0, end).matchAll(fences)) {
    if (match.index >= start && !reachedWindow) {
      opening = fence ? `${fence.marker}${fence.info}\n` : "";
      reachedWindow = true;
    }
    if (!fence) fence = { marker: match[1], info: match[2] };
    else if (match[1][0] === fence.marker[0] && match[1].length >= fence.marker.length && !match[2].trim()) fence = undefined;
  }
  if (!reachedWindow) opening = fence ? `${fence.marker}${fence.info}\n` : "";
  return { start, end, text: `${opening}${source.slice(start, end)}${fence ? `\n${fence.marker}` : ""}`, before: start > 0, after: end < source.length };
}

export function rehypeSessionHighlight({query, skipCode = true}: {query: string; skipCode?: boolean}) {
  const terms=query.trim().split(/\s+/).filter(Boolean).map(s=>s.replace(/[.*+?^${}()|[\]\\]/g,"\\$&"));
  return (tree:Node)=>{
    if(!terms.length)return;
    const pattern=new RegExp(`(${terms.join("|")})`,"gi");
    const visit=(node:Node)=>{
      // Code blocks do their own highlighting after worker tokenization.
      if(!node.children || node.tagName==="mark" || node.tagName==="math" || (Array.isArray(node.properties?.className) && node.properties.className.includes("katex")) || (skipCode && node.tagName==="pre"))return;
      node.children=node.children.flatMap(child=>{
        if(child.type!=="text"||!child.value){visit(child);return [child];}
        return child.value.split(pattern).filter(part=>part.length>0).map(part=>{
          pattern.lastIndex=0;
          return pattern.test(part) ? {type:"element",tagName:"mark",properties:{},children:[{type:"text",value:part}]} : {type:"text",value:part};
        });
      });
    };
    visit(tree);
  };
}
