import { describe, expect, it } from "vitest";
import { sessionDisplayTitle, sessionExcerpt } from "../lib/session-presentation";

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
});
