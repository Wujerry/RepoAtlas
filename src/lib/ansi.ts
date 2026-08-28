export interface AnsiSpan {
  text: string;
  className?: string;
}

const ANSI_PATTERN = /\u001b\[([0-9;]*)m/g;
const OSC_PATTERN = /\u001b\][\s\S]*?(?:\u0007|\u001b\\)/g;
const OTHER_ESCAPE_PATTERN = /\u001b(?:[@-Z\\-_]|\[[0-9:;<=>?]*[ -/]*[@-~])/g;

const COLOR_CODES: Record<number, string> = {
  30: "ansi-black",
  31: "ansi-red",
  32: "ansi-green",
  33: "ansi-yellow",
  34: "ansi-blue",
  35: "ansi-magenta",
  36: "ansi-cyan",
  37: "ansi-white",
  90: "ansi-bright-black",
  91: "ansi-bright-red",
  92: "ansi-bright-green",
  93: "ansi-bright-yellow",
  94: "ansi-bright-blue",
  95: "ansi-bright-magenta",
  96: "ansi-bright-cyan",
  97: "ansi-bright-white",
};

function classNameFromCodes(codes: number[]): string | undefined {
  const classes: string[] = [];
  let color: string | undefined;
  for (const code of codes) {
    if (code === 1) classes.push("ansi-bold");
    else if (code === 2) classes.push("ansi-dim");
    else if (code === 3) classes.push("ansi-italic");
    else if (code === 4) classes.push("ansi-underline");
    else if (COLOR_CODES[code]) color = COLOR_CODES[code];
  }
  if (color) classes.push(color);
  return classes.length ? classes.join(" ") : undefined;
}

function applyCodes(current: number[], codes: number[]): number[] {
  let next = [...current];
  for (const code of codes) {
    if (code === 0) next = [];
    else if (code === 22) next = next.filter((item) => item !== 1 && item !== 2);
    else if (code === 23) next = next.filter((item) => item !== 3);
    else if (code === 24) next = next.filter((item) => item !== 4);
    else if (code === 39) next = next.filter((item) => !COLOR_CODES[item]);
    else if (COLOR_CODES[code]) next = [...next.filter((item) => !COLOR_CODES[item]), code];
    else if (!next.includes(code)) next.push(code);
  }
  return next;
}

export function stripAnsi(input: string): string {
  return input.replace(OSC_PATTERN, "").replace(ANSI_PATTERN, "").replace(OTHER_ESCAPE_PATTERN, "");
}

export function parseAnsi(input: string): AnsiSpan[] {
  const source = input.replace(OSC_PATTERN, "");
  const spans: AnsiSpan[] = [];
  let lastIndex = 0;
  let current: number[] = [];
  const token = /\u001b\[([0-9;]*)m|\u001b(?:[@-Z\\-_]|\[[0-9:;<=>?]*[ -/]*[@-~])/g;
  for (const match of source.matchAll(token)) {
    const index = match.index ?? 0;
    if (index > lastIndex) spans.push({ text: source.slice(lastIndex, index), className: classNameFromCodes(current) });
    if (match[1] !== undefined) {
      const raw = match[1];
      const codes = raw === "" ? [0] : raw.split(";").map((value) => Number(value || 0));
      current = applyCodes(current, codes);
    }
    lastIndex = index + match[0].length;
  }
  if (lastIndex < source.length) spans.push({ text: source.slice(lastIndex), className: classNameFromCodes(current) });
  return spans.filter((span) => span.text.length > 0);
}

