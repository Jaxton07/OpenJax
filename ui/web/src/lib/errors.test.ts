import { describe, expect, it } from "vitest";
import { humanizeError } from "./errors";

describe("humanizeError", () => {
  it("maps known gateway codes", () => {
    const text = humanizeError({
      code: "UNAUTHENTICATED",
      message: "bad key",
      status: 401,
      retryable: false
    });

    expect(text).toContain("API Key");
  });

  it("falls back to generic error", () => {
    expect(humanizeError(new Error("boom"))).toBe("boom");
  });
});
