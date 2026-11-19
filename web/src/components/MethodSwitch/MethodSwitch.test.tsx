import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { MethodSwitch } from "./MethodSwitch";

describe("MethodSwitch", () => {
  it("draws nothing for a single method", () => {
    const { container } = render(
      <MethodSwitch methods={["password"]} value="password" onChange={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows each method in words and reports the choice", async () => {
    const onChange = vi.fn();
    render(<MethodSwitch methods={["password", "code"]} value="password" onChange={onChange} />);
    expect(screen.getByRole("tab", { name: "Password" })).toHaveAttribute("aria-selected", "true");
    await userEvent.click(screen.getByRole("tab", { name: "Email code" }));
    expect(onChange).toHaveBeenCalledWith("code");
  });
});
