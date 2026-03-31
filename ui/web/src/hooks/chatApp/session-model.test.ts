import { describe, expect, it } from "vitest";
import { PLACEHOLDER_SESSION_TITLE, resolveSessionTitle, summarizeTitle } from "./session-model";

describe("summarizeTitle", () => {
  it("truncates by code points and appends ellipsis when exceeding 24 chars", () => {
    const input = "😀".repeat(25);
    expect(summarizeTitle(input)).toBe(`${"😀".repeat(24)}...`);
  });

  it("normalizes whitespace and keeps exactly 24 chars without ellipsis", () => {
    const input = `   ${"你".repeat(24)}   `;
    expect(summarizeTitle(input)).toBe("你".repeat(24));
  });

  it("collapses internal whitespace before truncation", () => {
    expect(summarizeTitle("a\t\tb \n c")).toBe("a b c");
  });
});

describe("resolveSessionTitle", () => {
  it("prioritizes remote title", () => {
    const resolved = resolveSessionTitle({
      remoteTitle: "远端",
      localTitle: "本地",
      localIsPlaceholderTitle: false,
      inferredTitle: "推导"
    });
    expect(resolved).toEqual({ title: "远端", isPlaceholderTitle: false });
  });

  it("uses non-placeholder local title when remote title is absent", () => {
    const resolved = resolveSessionTitle({
      localTitle: "本地",
      localIsPlaceholderTitle: false,
      inferredTitle: "推导"
    });
    expect(resolved).toEqual({ title: "本地", isPlaceholderTitle: false });
  });

  it("uses inferred title when local title is placeholder", () => {
    const resolved = resolveSessionTitle({
      localTitle: PLACEHOLDER_SESSION_TITLE,
      localIsPlaceholderTitle: true,
      inferredTitle: "推导"
    });
    expect(resolved).toEqual({ title: "推导", isPlaceholderTitle: false });
  });

  it("falls back to placeholder when all candidates are empty", () => {
    const resolved = resolveSessionTitle({});
    expect(resolved).toEqual({ title: PLACEHOLDER_SESSION_TITLE, isPlaceholderTitle: true });
  });
});
