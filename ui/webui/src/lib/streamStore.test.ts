import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { streamStore } from "./streamStore";

describe("streamStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb: FrameRequestCallback) => {
      setTimeout(() => cb(performance.now()), 1);
      return 1;
    });
  });

  afterEach(() => {
    streamStore.__resetForTests();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("deduplicates older seq deltas", () => {
    streamStore.start("sess", "turn", 1);
    streamStore.append("sess", "turn", "A", 2);
    streamStore.append("sess", "turn", "B", 2);

    vi.runAllTimers();

    const snapshot = streamStore.getSnapshot("sess", "turn");
    expect(snapshot.content).toBe("A");
    expect(snapshot.lastEventSeq).toBe(2);
  });

  it("commits buffered delta on raf", () => {
    streamStore.start("sess", "turn", 1);
    streamStore.append("sess", "turn", "hel", 2);
    streamStore.append("sess", "turn", "lo", 3);

    vi.runAllTimers();

    const snapshot = streamStore.getSnapshot("sess", "turn");
    expect(snapshot.content).toBe("hello");
  });

  it("prefers completion content when present", () => {
    streamStore.start("sess", "turn", 1);
    streamStore.append("sess", "turn", "draft", 2);
    vi.runAllTimers();

    streamStore.complete("sess", "turn", "final", 3);

    const snapshot = streamStore.getSnapshot("sess", "turn");
    expect(snapshot.content).toBe("final");
    expect(snapshot.isActive).toBe(false);
  });
});
