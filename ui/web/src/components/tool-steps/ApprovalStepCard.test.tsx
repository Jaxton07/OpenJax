import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ApprovalStepCard from "./ApprovalStepCard";

describe("ApprovalStepCard", () => {
  it("renders without actions when pending approval is missing", () => {
    render(
      <ApprovalStepCard
        step={{
          id: "step_1",
          type: "approval",
          title: "approval",
          status: "success",
          time: "2026-01-01T00:00:00Z"
        }}
        onResolve={() => {}}
      />
    );

    expect(screen.getByTestId("approval-step-card")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "允许" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "拒绝" })).not.toBeInTheDocument();
  });

  it("calls onResolve and disables actions while submitting", async () => {
    const onResolve = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    render(
      <ApprovalStepCard
        step={{
          id: "step_2",
          type: "approval",
          title: "approval",
          status: "waiting",
          time: "2026-01-01T00:00:00Z",
          approvalId: "approval_1"
        }}
        pendingApproval={{ approvalId: "approval_1", toolName: "shell" }}
        onResolve={onResolve}
      />
    );

    const approveBtn = screen.getByRole("button", { name: "允许" });
    const rejectBtn = screen.getByRole("button", { name: "拒绝" });
    fireEvent.click(approveBtn);

    expect(onResolve).toHaveBeenCalledWith({ approvalId: "approval_1", toolName: "shell" }, true);
    expect(approveBtn).toBeDisabled();
    expect(rejectBtn).toBeDisabled();

    await waitFor(() => expect(approveBtn).not.toBeDisabled());
  });
});
