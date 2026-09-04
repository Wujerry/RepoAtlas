/// <reference lib="webworker" />

import { createHighlighterCore, type LanguageInput, type ThemedToken } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

type HighlightRequest = {
  type: "highlight";
  id: number;
  code: string;
  language: string;
  theme: "github-light" | "github-dark";
};

type CancelRequest = { type: "cancel"; id: number };

type HighlightResponse = {
  id: number;
  lines?: ThemedToken[][];
  error?: string;
};

const languageLoaders: Record<string, () => Promise<LanguageInput>> = {
  bash: () => import("shiki/langs/bash.mjs").then((module) => module.default),
  c: () => import("shiki/langs/c.mjs").then((module) => module.default),
  cpp: () => import("shiki/langs/cpp.mjs").then((module) => module.default),
  csharp: () => import("shiki/langs/csharp.mjs").then((module) => module.default),
  css: () => import("shiki/langs/css.mjs").then((module) => module.default),
  dart: () => import("shiki/langs/dart.mjs").then((module) => module.default),
  diff: () => import("shiki/langs/diff.mjs").then((module) => module.default),
  docker: () => import("shiki/langs/docker.mjs").then((module) => module.default),
  go: () => import("shiki/langs/go.mjs").then((module) => module.default),
  html: () => import("shiki/langs/html.mjs").then((module) => module.default),
  java: () => import("shiki/langs/java.mjs").then((module) => module.default),
  javascript: () => import("shiki/langs/javascript.mjs").then((module) => module.default),
  jsx: () => import("shiki/langs/jsx.mjs").then((module) => module.default),
  json: () => import("shiki/langs/json.mjs").then((module) => module.default),
  kotlin: () => import("shiki/langs/kotlin.mjs").then((module) => module.default),
  less: () => import("shiki/langs/less.mjs").then((module) => module.default),
  make: () => import("shiki/langs/make.mjs").then((module) => module.default),
  markdown: () => import("shiki/langs/markdown.mjs").then((module) => module.default),
  php: () => import("shiki/langs/php.mjs").then((module) => module.default),
  powershell: () => import("shiki/langs/powershell.mjs").then((module) => module.default),
  python: () => import("shiki/langs/python.mjs").then((module) => module.default),
  ruby: () => import("shiki/langs/ruby.mjs").then((module) => module.default),
  rust: () => import("shiki/langs/rust.mjs").then((module) => module.default),
  scss: () => import("shiki/langs/scss.mjs").then((module) => module.default),
  sql: () => import("shiki/langs/sql.mjs").then((module) => module.default),
  svelte: () => import("shiki/langs/svelte.mjs").then((module) => module.default),
  swift: () => import("shiki/langs/swift.mjs").then((module) => module.default),
  toml: () => import("shiki/langs/toml.mjs").then((module) => module.default),
  tsx: () => import("shiki/langs/tsx.mjs").then((module) => module.default),
  typescript: () => import("shiki/langs/typescript.mjs").then((module) => module.default),
  vue: () => import("shiki/langs/vue.mjs").then((module) => module.default),
  xml: () => import("shiki/langs/xml.mjs").then((module) => module.default),
  yaml: () => import("shiki/langs/yaml.mjs").then((module) => module.default),
};

const highlighterPromise = createHighlighterCore({
  engine: createJavaScriptRegexEngine(),
  themes: [
    import("shiki/themes/github-light.mjs").then((module) => module.default),
    import("shiki/themes/github-dark.mjs").then((module) => module.default),
  ],
  langs: [],
});

const loadedLanguages = new Set<string>();
const queuedRequests: HighlightRequest[] = [];
const canceledRequests = new Set<number>();
let queueTimer: ReturnType<typeof setTimeout> | undefined;
let processing = false;

async function processQueuedRequests() {
  queueTimer = undefined;
  if (processing) return;
  processing = true;
  try {
    while (queuedRequests.length > 0) {
      const request = queuedRequests.shift();
      if (!request || canceledRequests.delete(request.id)) continue;
      const { id, code, language, theme } = request;
      try {
        const highlighter = await highlighterPromise;
        if (canceledRequests.delete(id)) continue;
        const loader = languageLoaders[language];
        let resolvedLanguage = "text";
        if (loader) {
          if (!loadedLanguages.has(language)) {
            await highlighter.loadLanguage(await loader());
            loadedLanguages.add(language);
          }
          resolvedLanguage = language;
        }
        if (canceledRequests.delete(id)) continue;
        const result = highlighter.codeToTokens(code, {
          lang: resolvedLanguage,
          theme,
          tokenizeMaxLineLength: 20_000,
          tokenizeTimeLimit: 100,
        });
        if (!canceledRequests.delete(id)) {
          self.postMessage({ id, lines: result.tokens } satisfies HighlightResponse);
        }
      } catch (error) {
        if (!canceledRequests.delete(id)) {
          self.postMessage({ id, error: String(error) } satisfies HighlightResponse);
        }
      }
    }
  } finally {
    processing = false;
    if (queuedRequests.length > 0 && queueTimer === undefined) {
      queueTimer = setTimeout(() => void processQueuedRequests(), 20);
    }
  }
}

self.onmessage = (event: MessageEvent<HighlightRequest | CancelRequest>) => {
  if (event.data.type === "cancel") {
    canceledRequests.add(event.data.id);
    const queuedIndex = queuedRequests.findIndex((request) => request.id === event.data.id);
    if (queuedIndex >= 0) {
      queuedRequests.splice(queuedIndex, 1);
      canceledRequests.delete(event.data.id);
    }
    return;
  }
  queuedRequests.push(event.data);
  if (!processing && queueTimer === undefined) {
    queueTimer = setTimeout(() => void processQueuedRequests(), 20);
  }
};
