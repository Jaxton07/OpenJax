import { describe, expect, it } from "vitest";
import { humanizeError, isAuthenticationError } from "./errors";

describe("humanizeError", () => {
  it("maps known gateway codes", () => {
    const text = humanizeError({
      code: "UNAUTHENTICATED",
      message: "bad key",
      status: 401,
      retryable: false
    });

    expect(text).toContain("Owner Key");
  });

  it("maps conflict to busy-turn guidance", () => {
    const text = humanizeError({
      code: "CONFLICT",
      message: "another turn is still running",
      status: 409,
      retryable: false
    });
    expect(text).toBe("Please wait for the current response to finish.");
  });

  it("falls back to generic error", () => {
    expect(humanizeError(new Error("boom"))).toBe("boom");
  });
});

describe("isAuthenticationError", () => {
  it("matches unauthenticated gateway error code", () => {
    expect(
      isAuthenticationError({
        code: "UNAUTHENTICATED",
        message: "bad key",
        status: 401,
        retryable: false
      })
    ).toBe(true);
  });

  it("matches forbidden status", () => {
    expect(
      isAuthenticationError({
        message: "forbidden",
        status: 403
      })
    ).toBe(true);
  });

  it("returns false for non-auth errors", () => {
    expect(
      isAuthenticationError({
        code: "NOT_FOUND",
        message: "missing",
        status: 404
      })
    ).toBe(false);
  });
});
