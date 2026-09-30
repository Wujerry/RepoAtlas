import { describe, expect, it } from "vitest";
import { sessionDisplayText, sessionDisplayTitle, sessionExcerpt } from "../lib/session-presentation";

describe("session display excerpts", () => {
  it("decodes exported entities and flattens Markdown without interpreting HTML", () => {
    expect(sessionExcerpt("**New app**&#x20;[requirements](https://example.com)\n|---|---|\n&amp; details")).toBe("New app requirements & details");
    expect(sessionExcerpt("&lt;script&gt;alert(1)&lt;/script&gt;")).toBe("<script>alert(1)</script>");
    expect(sessionExcerpt("&#x110000; &#xd800;")).toBe("&#x110000; &#xd800;");
  });
  it("keeps client setup and attachment wrappers out of display excerpts", () => {
    expect(sessionExcerpt("<recommended_plugins>client setup</recommended_plugins>")).toBe("");
    expect(sessionExcerpt("# Files mentioned by the user:\nimage.png\n## My request:\nFix the navigation")).toBe("Fix the navigation");
  });
  it("does not show a cached reasoning configuration as a title", () => {
    expect(sessionDisplayTitle("auto", "Untitled session")).toBe("Untitled session");
    expect(sessionDisplayTitle("Automatic retry", "Untitled session")).toBe("Automatic retry");
  });
  it("removes known internal envelopes, including clipped cached notification titles", () => {
    expect(sessionExcerpt('<external_codex_apps_open_page>{"page_id":null}</external_codex_apps_open_page>')).toBe("");
    expect(sessionDisplayTitle('<subagent_notification>{"status":"Changed', "Untitled")).toBe("Untitled");
    expect(sessionDisplayText('<environment_context>cwd</environment_context>\n<external_codex_apps_open_page>null</external_codex_apps_open_page>\nFix login')).toBe("Fix login");
  });
  it("keeps ordinary XML, code, and quoted discussion of client markers", () => {
    expect(sessionDisplayText('<div>Hello</div>')).toBe('<div>Hello</div>');
    expect(sessionExcerpt('Explain the <subagent_notification> marker')).toBe('Explain the <subagent_notification> marker');
    expect(sessionDisplayText('  User text\n  **unchanged**')).toBe('  User text\n  **unchanged**');
  });
  it("uses the literal request following a pasted quote without generating a summary", () => {
    expect(sessionDisplayTitle('‘产品发展建议很长’，先完成第3点', "Untitled")).toBe("先完成第3点");
    expect(sessionDisplayTitle('“Standalone quote”', "Untitled")).toBe('“Standalone quote”');
  });
});
