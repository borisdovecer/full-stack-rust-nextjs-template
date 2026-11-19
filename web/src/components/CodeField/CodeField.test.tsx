import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CodeField, RESEND_SECONDS, SendAgain } from "./CodeField";

describe("CodeField", () => {
  it("asks for a numeric one-time code and keeps only six digits", () => {
    const onChange = vi.fn();
    render(<CodeField value="" onChange={onChange} />);
    const input = screen.getByLabelText("Code");
    expect(input).toHaveAttribute("inputmode", "numeric");
    expect(input).toHaveAttribute("autocomplete", "one-time-code");
    fireEvent.change(input, { target: { value: " 123-456 7" } });
    expect(onChange).toHaveBeenCalledWith({ target: { value: "123456" } });
  });
});

describe("SendAgain", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("waits before another code can be sent", async () => {
    const onSend = vi.fn(() => Promise.resolve());
    render(<SendAgain onSend={onSend} />);
    expect(
      screen.getByRole("button", { name: `Send a new code in ${RESEND_SECONDS} s` }),
    ).toBeDisabled();
    for (let i = 0; i < RESEND_SECONDS; i++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1000);
      });
    }
    const button = screen.getByRole("button", { name: "Send a new code" });
    expect(button).toBeEnabled();
    await act(async () => button.click());
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(
      screen.getByRole("button", { name: `Send a new code in ${RESEND_SECONDS} s` }),
    ).toBeDisabled();
  });
});
