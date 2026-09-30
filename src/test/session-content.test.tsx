import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SessionContent, SessionSnippet } from "../components/ui/session-content";
import { sessionMarkdownWindow } from "../lib/session-markdown";
import { dictionaries,type MessageKey } from "../i18n";
const openUrl=vi.hoisted(()=>vi.fn());
const highlight=vi.hoisted(()=>vi.fn());
vi.mock("@tauri-apps/plugin-opener",()=>({openUrl}));
vi.mock("../lib/shiki-client",()=>({highlightCode: highlight}));
const t=(key:MessageKey)=>dictionaries.en[key];
const copy=vi.fn();
beforeEach(()=>{vi.clearAllMocks();copy.mockResolvedValue(undefined);openUrl.mockResolvedValue(undefined);Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:copy}});});
afterEach(()=>vi.unstubAllGlobals());
it("renders readable Markdown and highlights literal terms across formatting",()=>{
  const {container}=render(<SessionContent content={'## Fix\n\n- **a+b** and `a+b`\n\n```ts\nconst x = "a+b";\n```'} query="a+b" t={t}/>);
  expect(screen.getByRole("heading",{name:"Fix"})).toBeInTheDocument();
  expect(container.querySelectorAll("mark")).toHaveLength(3);expect(container.querySelectorAll("li")).toHaveLength(1);
});
it("does not load remote images or execute HTML, and only opens safe links on click",async()=>{
  const {container}=render(<SessionContent content={'![screen](https://untrusted.invalid/track.png)\n\n<script>alert(1)</script>\n\n[docs](https://example.com) [bad](javascript:alert(1))'} t={t}/>);
  expect(container.querySelector("img,script")).toBeNull();expect(openUrl).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("link",{name:"docs"}));await waitFor(()=>expect(openUrl).toHaveBeenCalledWith("https://example.com"));
  expect(screen.queryByRole("link",{name:"bad"})).not.toBeInTheDocument();
});
it("folds client envelopes and retains the complete original for viewing and copying",async()=>{
  const content='<subagent_notification>{"status":"completed"}</subagent_notification>';
  const {container}=render(<SessionContent content={content} t={t} copy/>);
  expect(screen.getByText(t("sessionTechnical"))).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:t("sessionRaw")}));expect(container.querySelector("pre")).toHaveTextContent(content);
  fireEvent.click(screen.getByRole("button",{name:t("qsCopyMessage")}));await waitFor(()=>expect(copy).toHaveBeenCalledWith(content));
});
it("bounds long messages while locating late matches and preserving full copy",async()=>{
  const content="prefix ".repeat(10000)+"NEEDLE";
  const {container}=render(<SessionContent content={content} query="NEEDLE" limit={360} t={t} copy/>);
  expect(container.querySelector("mark")).toHaveTextContent("NEEDLE");expect(container.textContent!.length).toBeLessThan(600);
  fireEvent.click(screen.getByRole("button",{name:t("qsExpand")}));expect(container.textContent!.length).toBeLessThan(41000);
  fireEvent.click(screen.getByRole("button",{name:t("qsCopyMessage")}));await waitFor(()=>expect(copy).toHaveBeenCalledWith(content));
});

it("renders tables, task lists, safe disclosure HTML and math without fetching content",()=>{
  const {container}=render(<SessionContent content={'| Check | Result |\n| --- | --- |\n| Cache | **kept** |\n\n- [x] Tested\n- [ ] Pending\n\n<details><summary>Details</summary><kbd>Ctrl</kbd> + K</details>\n\n$$x^2 + y^2$$\n\n<iframe src="https://bad.invalid"></iframe>'} t={t}/>);
  expect(screen.getByRole("table")).toBeInTheDocument();
  expect(screen.getAllByRole("checkbox")).toHaveLength(2);
  expect(screen.getAllByRole("checkbox")[0]).toBeDisabled();
  expect(screen.getByText("Details").closest("details")).toBeInTheDocument();
  expect(container.querySelector(".katex")).toBeInTheDocument();
  expect(container.querySelector("iframe,img,script")).toBeNull();
});

it("copies just the code and can wrap long lines independently of original-message copy",async()=>{
  const content='## Fix\n\n```ts\nconst value = "literal **text**";\n```';
  const {container}=render(<SessionContent content={content} t={t} copy/>);
  expect(screen.getByText("ts")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:t("sessionWrap")}));
  expect(container.querySelector("pre")).toHaveClass("is-wrapped");
  fireEvent.click(screen.getByRole("button",{name:t("sessionCopyCode")}));
  await waitFor(()=>expect(copy).toHaveBeenCalledWith('const value = "literal **text**";'));
  fireEvent.click(screen.getByRole("button",{name:t("qsCopyMessage")}));
  await waitFor(()=>expect(copy).toHaveBeenCalledWith(content));
});

it("keeps Markdown around late search matches and preserves clipped fenced-code language",()=>{
  const source='Earlier context. '.repeat(150)+'\n\n```typescript\n'+ 'const other = 0;\n'.repeat(100)+'const NEEDLE = true;\n'+ 'const after = 1;\n'.repeat(100)+'```';
  const {container}=render(<SessionContent content={source} query="NEEDLE" limit={500} t={t}/>);
  expect(screen.getByText("NEEDLE",{selector:"mark"})).toBeInTheDocument();
  expect(screen.getByText("typescript")).toBeInTheDocument();
  expect(container.querySelector(".session-code-block pre")).toHaveTextContent("const NEEDLE = true;");
  expect(container.querySelector(".session-source")).toBeNull();
  expect(container.textContent!.length).toBeLessThan(1000);
});

it("reads messages beyond the former 40k limit in bounded consecutive sections",()=>{
  const content='text '.repeat(11000)+'THE_END';
  const {container}=render(<SessionContent content={content} t={t} limit={1000}/>);
  fireEvent.click(screen.getByRole("button",{name:t("qsExpand")}));
  for(let page=0;page<4;page++) {
    expect(container.querySelector(".session-markdown")!.textContent!.length).toBeLessThanOrEqual(12000);
    fireEvent.click(screen.getByRole("button",{name:t("sessionNextPart")}));
  }
  expect(container.querySelector(".session-markdown")).toHaveTextContent("THE_END");
  expect(screen.getByRole("button",{name:t("sessionNextPart")})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:t("sessionPreviousPart")}));
  expect(container.querySelector(".session-markdown")).not.toHaveTextContent("THE_END");
});

it("does not duplicate complete fences and preserves every source character across windows",()=>{
  const source='Intro\n\n```js\n'+ 'const value = 123;\n'.repeat(100)+'```\n\nEnd';
  const first=sessionMarkdownWindow(source,"",600);
  expect(first.text.match(/```js/g)).toHaveLength(1);
  let offset=0; let collected="";
  while(offset<source.length) {
    const part=sessionMarkdownWindow(source,"",600,offset);
    collected+=source.slice(part.start,part.end);
    offset=part.end;
  }
  expect(collected).toBe(source);
});

it("renders inline formatting in every result-row snippet without nested interactive elements",()=>{
  const {container}=render(<button><SessionSnippet content={'## **Fix** `login.ts`\n\n[guide](https://example.com)\n\n- [x] Tested\n\n| Key | Value |\n| --- | --- |\n| **cache** | kept |'} query="cache"/></button>);
  expect(container.querySelector("strong")).toHaveTextContent("Fix");
  expect(container.querySelector("code")).toHaveTextContent("login.ts");
  expect(container.querySelector("mark")).toHaveTextContent("cache");
  expect(container.querySelector("button button,a,input,p,table,h2,img")).toBeNull();
});

it("highlights code only when visible, resolves aliases, and cancels when the reader leaves",async()=>{
  let notify: IntersectionObserverCallback | undefined;
  vi.stubGlobal("IntersectionObserver",class { constructor(callback:IntersectionObserverCallback){notify=callback;} observe(){} disconnect(){} });
  highlight.mockResolvedValue([[{content:"const x = 1;",offset:0,color:"#a00"}]]);
  const {unmount}=render(<SessionContent content={'```ts\nconst x = 1;\n```'} t={t}/>);
  expect(highlight).not.toHaveBeenCalled();
  notify?.([{isIntersecting:true} as IntersectionObserverEntry],{} as IntersectionObserver);
  await waitFor(()=>expect(highlight).toHaveBeenCalledWith("const x = 1;","typescript",false,expect.any(AbortSignal)));
  const signal=highlight.mock.calls[0][3] as AbortSignal;
  unmount();expect(signal.aborted).toBe(true);
});
