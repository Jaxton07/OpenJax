import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { streamRenderStore } from "./streamRenderStore";

describe("streamRenderStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      return setTimeout(() => cb(performance.now()), 16) as unknown as number;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => clearTimeout(id));
    streamRenderStore.__dangerousResetForTests();
  });

  afterEach(() => {
    streamRenderStore.__dangerousResetForTests();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("appends deltas in order after frame commit", () => {
    streamRenderStore.start("sess", "turn_1", "m1", 1);
    streamRenderStore.append("sess", "turn_1", "你", 2);
    streamRenderStore.append("sess", "turn_1", "好", 3);
    expect(streamRenderStore.getSnapshot("sess", "turn_1").content).toBe("");
    vi.advanceTimersByTime(17);
    const snapshot = streamRenderStore.getSnapshot("sess", "turn_1");
    expect(snapshot.content).toBe("你好");
    expect(snapshot.lastEventSeq).toBe(3);
    expect(snapshot.isActive).toBe(true);
  });

  it("drops stale sequence events", () => {
    streamRenderStore.start("sess", "turn_1", "m1", 10, "A");
    streamRenderStore.append("sess", "turn_1", "B", 9);
    vi.advanceTimersByTime(17);
    const snapshot = streamRenderStore.getSnapshot("sess", "turn_1");
    expect(snapshot.content).toBe("A");
    expect(snapshot.lastEventSeq).toBe(10);
  });

  it("completes with final content and deactivates", () => {
    streamRenderStore.start("sess", "turn_1", "m1", 1);
    streamRenderStore.append("sess", "turn_1", "你", 2);
    vi.advanceTimersByTime(17);
    streamRenderStore.complete("sess", "turn_1", "你好！", 3);
    const snapshot = streamRenderStore.getSnapshot("sess", "turn_1");
    expect(snapshot.content).toBe("你好！");
    expect(snapshot.isActive).toBe(false);
    expect(snapshot.lastEventSeq).toBe(3);
  });

  it("resets buffered content when a new response segment starts with different message id", () => {
    streamRenderStore.start("sess", "turn_1", "assistant_turn_1_resp_1", 1);
    streamRenderStore.append("sess", "turn_1", "第一段", 2);
    vi.advanceTimersByTime(17);
    expect(streamRenderStore.getSnapshot("sess", "turn_1").content).toBe("第一段");

    streamRenderStore.start("sess", "turn_1", "assistant_turn_1_resp_2", 3, "");
    streamRenderStore.append("sess", "turn_1", "第二段", 4);
    vi.advanceTimersByTime(17);
    const snapshot = streamRenderStore.getSnapshot("sess", "turn_1");
    expect(snapshot.content).toBe("第二段");
    expect(snapshot.messageId).toBe("assistant_turn_1_resp_2");
  });

  it("isolates concurrent segment snapshots by stream key", () => {
    streamRenderStore.start("sess", "turn_1", "assistant_turn_1_resp_1", 1, "", "resp_1");
    streamRenderStore.append("sess", "turn_1", "A", 2, "resp_1");
    vi.advanceTimersByTime(17);

    streamRenderStore.start("sess", "turn_1", "assistant_turn_1_resp_2", 3, "", "resp_2");
    streamRenderStore.append("sess", "turn_1", "B", 4, "resp_2");
    vi.advanceTimersByTime(17);

    const resp1 = streamRenderStore.getSnapshot("sess", "turn_1", "resp_1");
    const resp2 = streamRenderStore.getSnapshot("sess", "turn_1", "resp_2");
    expect(resp1.content).toBe("A");
    expect(resp2.content).toBe("B");
  });

  it("notifies once for multiple deltas in same frame", () => {
    const listener = vi.fn();
    streamRenderStore.start("sess", "turn_1", "m1", 1);
    const unsubscribe = streamRenderStore.subscribe("sess", "turn_1", listener);
    listener.mockClear();
    streamRenderStore.append("sess", "turn_1", "你", 2);
    streamRenderStore.append("sess", "turn_1", "好", 3);
    streamRenderStore.append("sess", "turn_1", "！", 4);
    vi.advanceTimersByTime(17);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(streamRenderStore.getSnapshot("sess", "turn_1").content).toBe("你好！");
    unsubscribe();
  });

  it("clears turn snapshot", () => {
    streamRenderStore.start("sess", "turn_1", "m1", 1, "hello");
    streamRenderStore.clear("sess", "turn_1");
    const snapshot = streamRenderStore.getSnapshot("sess", "turn_1");
    expect(snapshot.content).toBe("");
    expect(snapshot.version).toBe(0);
    expect(snapshot.isActive).toBe(false);
  });
});
