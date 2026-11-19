import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Markdown, resolveHref } from "./Markdown";

describe("Markdown", () => {
  it("renders headings, tables and code", () => {
    render(<Markdown>{"# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n`x`"}</Markdown>);
    expect(screen.getByRole("heading", { level: 1, name: "Title" })).toBeInTheDocument();
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByText("x").tagName).toBe("CODE");
  });

  it("does not render raw HTML", () => {
    const { container } = render(<Markdown>{'<img src=x onerror="alert(1)">\n\nhello'}</Markdown>);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("keeps doc links inside help and opens external ones apart", () => {
    expect(resolveHref("auth.md", "/help")).toEqual({ href: "/help/auth", external: false });
    expect(resolveHref("auth.md#cookie", "/help")).toEqual({
      href: "/help/auth#cookie",
      external: false,
    });
    expect(resolveHref("https://example.com", "/help").external).toBe(true);
    expect(resolveHref("javascript:alert(1)", "/help").external).toBe(true);
  });
});
