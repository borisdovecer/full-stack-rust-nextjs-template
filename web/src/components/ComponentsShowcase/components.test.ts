// @vitest-environment node
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import { REUSABLE, VIEWS } from "./ComponentsShowcase";

const dir = resolve(__dirname, "..");
const folders = readdirSync(dir, { withFileTypes: true })
  .filter((d) => d.isDirectory() && d.name !== "ui")
  .map((d) => d.name);
const showcase = readFileSync(resolve(__dirname, "ComponentsShowcase.tsx"), "utf8");

describe("components page", () => {
  it("every component folder is listed as reusable or as a view", () => {
    const unlisted = folders.filter((f) => !REUSABLE.includes(f) && !VIEWS.includes(f));
    expect(unlisted).toEqual([]);
  });

  it("every reusable component appears on the page", () => {
    const missing = REUSABLE.filter((name) => !showcase.includes(name));
    expect(missing).toEqual([]);
  });

  it("shadcn primitives use the variants Base UI actually sets", () => {
    const ui = resolve(dir, "ui");
    const offenders = readdirSync(ui).filter((f) => {
      const text = readFileSync(resolve(ui, f), "utf8");
      // Base UI sets data-orientation="…", never a bare data-horizontal attribute.
      return /data-(horizontal|vertical)[:/]/.test(text) || /from "cn"/.test(text);
    });
    expect(offenders).toEqual([]);
  });

  it("every component folder has a file of the same name", () => {
    for (const f of folders) {
      expect(() => readFileSync(resolve(dir, f, `${f}.tsx`))).not.toThrow();
    }
  });
});
