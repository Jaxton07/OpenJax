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
    expect(screen.getByRole("region")).toHaveClass("step-body");
    expect(screen.getByRole("region")).not.toHaveClass("expanded");

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("region")).toHaveClass("expanded");

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("region")).not.toHaveClass("expanded");
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
    expect(dot).toHaveClass("step-dot--failed");
  });

  it("renders duration in seconds when under sixty seconds", () => {
    render(
      <ToolStepCard
        step={{
          id: "step_5",
          type: "tool",
          title: "read_file",
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
          title: "read_file",
          status: "success",
          time: "2026-01-01T00:00:00Z",
          durationSec: 125
        }}
      />
    );

    expect(screen.getByText("2m 5s")).toBeInTheDocument();
  });
});
