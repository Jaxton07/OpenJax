import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import PolicyLevelButton from "./PolicyLevelButton";

describe("PolicyLevelButton", () => {
  it("calls onChange when selecting a policy option", async () => {
    const onChange = vi.fn();
    render(<PolicyLevelButton level="ask" onChange={onChange} />);

    await userEvent.click(screen.getByRole("button", { name: /ask/i }));
    await userEvent.click(screen.getByRole("button", { name: /allow/i }));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("allow");
  });
});
