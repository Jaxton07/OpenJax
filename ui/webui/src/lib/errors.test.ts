import { describe, expect, it } from "vitest";
import { humanizeError } from "./errors";

describe("humanizeError", () => {
  it("maps known gateway code", () => {
    const msg = humanizeError({ code: "UNAUTHENTICATED", status: 401, message: "bad" });
    expect(msg).toContain("认证失败");
  });

  it("falls back to Error message", () => {
    const msg = humanizeError(new Error("x"));
    expect(msg).toBe("x");
  });
});
