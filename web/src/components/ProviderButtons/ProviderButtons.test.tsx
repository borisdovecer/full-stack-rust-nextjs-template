import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProviderButtons } from "./ProviderButtons";

describe("ProviderButtons", () => {
  it("links each provider to the API's start, with the intent", () => {
    render(<ProviderButtons providers={["google", "github"]} intent="register" />);
    expect(screen.getByRole("button", { name: "Continue with Google" })).toHaveAttribute(
      "href",
      "/api/auth/oauth/google/start?intent=register",
    );
    expect(screen.getByRole("button", { name: "Continue with GitHub" })).toHaveAttribute(
      "href",
      "/api/auth/oauth/github/start?intent=register",
    );
  });
});
