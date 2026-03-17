import { describe, expect, it } from "vitest";
import { sanitizeMarkdownContent } from "./markdown";

describe("sanitizeMarkdownContent", () => {
  it("escapes raw html tags", () => {
    const input = "safe<script>alert(1)</script><b>ok</b>";
    expect(sanitizeMarkdownContent(input)).toBe("safe&lt;script&gt;alert(1)&lt;/script&gt;&lt;b&gt;ok&lt;/b&gt;");
  });
});
