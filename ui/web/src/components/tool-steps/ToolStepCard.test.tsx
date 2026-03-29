import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import ToolStepCard from "./ToolStepCard";

describe("ToolStepCard", () => {
  it("is collapsed by default and toggles expanded state", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_1",
          type: "tool",
          title: "shell",
          status: "running",
          time: "2026-01-01T00:00:00Z",
          description: "run command"
        }}
      />
    );

    const toggleBtn = screen.getByRole("button", { name: /shell/i });
    expect(toggleBtn).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("region")).toHaveClass("step-detail");
    expect(screen.getByRole("region")).not.toHaveClass("open");

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("region")).toHaveClass("open");

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("region")).not.toHaveClass("open");
  });

  it("wires aria-controls to details region", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_2",
          type: "tool",
          title: "shell",
          status: "success",
          time: "2026-01-01T00:00:00Z",
          output: "ok"
        }}
      />
    );

    const toggleBtn = screen.getByRole("button", { name: /shell/i });
    const controlsId = toggleBtn.getAttribute("aria-controls");
    expect(controlsId).toBeTruthy();
    expect(document.getElementById(controlsId as string)).toBeInTheDocument();
  });

  it("does not render empty details region when no detail fields", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_3",
          type: "approval",
          title: "approval",
          status: "waiting",
          time: "2026-01-01T00:00:00Z"
        }}
      />
    );

    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });

  it("renders status dot class correctly", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_4",
          type: "summary",
          title: "error",
          status: "failed",
          time: "2026-01-01T00:00:00Z",
          output: "boom"
        }}
      />
    );

    const dot = document.querySelector(".step-dot");
    expect(dot).toHaveClass("step-dot-fail");
  });

  it("renders duration in seconds when under sixty seconds", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_5",
          type: "tool",
          title: "Read",
          status: "success",
          time: "2026-01-01T00:00:00Z",
          durationSec: 42
        }}
      />
    );

    expect(screen.getByText("42s")).toBeInTheDocument();
  });

  it("renders duration in minutes and seconds when over sixty seconds", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_6",
          type: "tool",
          title: "Read",
          status: "success",
          time: "2026-01-01T00:00:00Z",
          durationSec: 125
        }}
      />
    );

    expect(screen.getByText("2m 5s")).toBeInTheDocument();
  });

  it("renders target filename when provided", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_target",
          type: "tool",
          title: "Read",
          target: "/Users/ericw/work/code/ai/openJax/ui/web/src/types/chat.ts",
          status: "success",
          time: "2026-01-01T00:00:00Z"
        }}
      />
    );

    expect(screen.getByText("/Users/ericw/work/code/ai/openJax/ui/web/src/types/chat.ts")).toBeInTheDocument();
  });

  it("does not render target when not provided", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_no_target",
          type: "tool",
          title: "shell",
          status: "success",
          time: "2026-01-01T00:00:00Z"
        }}
      />
    );

    expect(document.querySelector(".step-target")).not.toBeInTheDocument();
  });

  it("renders structured shell metadata details", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_7",
          type: "tool",
          title: "Run Shell",
          status: "success",
          time: "2026-01-01T00:00:00Z",
          description: "Partial success (exit code 141)",
          meta: {
            backendSummary: "sandbox: sandbox-exec (macos_seatbelt)",
            riskSummary: "risk: mutating command ran unsandboxed",
            hint: "hint: detected skill trigger string in shell; use skill workflow steps"
          }
        }}
      />
    );

    const toggleBtn = screen.getByRole("button", { name: /run shell/i });
    fireEvent.click(toggleBtn);
    expect(screen.getByText("Partial success (exit code 141)")).toBeInTheDocument();
    expect(screen.getByText("sandbox: sandbox-exec (macos_seatbelt)")).toBeInTheDocument();
    expect(screen.getByText("risk: mutating command ran unsandboxed")).toBeInTheDocument();
    expect(
      screen.getByText("hint: detected skill trigger string in shell; use skill workflow steps")
    ).toBeInTheDocument();
  });
});
