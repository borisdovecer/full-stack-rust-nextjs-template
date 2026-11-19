// @vitest-environment node
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

/**
 * Every text colour is readable on the surface it sits on, in both themes (WCAG AA, 4.5:1).
 * A new text/surface token pair goes into PAIRS.
 */
const PAIRS: [text: string, surface: string][] = [
  ["foreground", "background"],
  ["muted-foreground", "background"],
  ["destructive", "background"],
  ["card-foreground", "card"],
  ["muted-foreground", "card"],
  ["popover-foreground", "popover"],
  ["primary-foreground", "primary"],
  ["secondary-foreground", "secondary"],
  ["muted-foreground", "muted"],
  ["accent-foreground", "accent"],
  ["destructive-foreground", "destructive"],
  ["success-foreground", "success"],
  ["warning-foreground", "warning"],
  ["sidebar-foreground", "sidebar"],
  ["sidebar-primary-foreground", "sidebar-primary"],
  ["sidebar-accent-foreground", "sidebar-accent"],
];

const css = readFileSync(resolve(__dirname, "globals.css"), "utf8");

function tokens(selector: string): Map<string, string> {
  const start = css.indexOf(`${selector} {`);
  const body = css.slice(start, css.indexOf("}", start));
  const out = new Map<string, string>();
  for (const m of body.matchAll(/--([a-z0-9-]+):\s*([^;]+);/g)) {
    if (m[1] && m[2]) out.set(m[1], m[2].trim());
  }
  return out;
}

/** oklch() to relative luminance, through OKLab and linear sRGB. */
function luminance(value: string): number {
  const m = value.match(/^oklch\(([\d.]+)\s+([\d.]+)\s+([\d.]+)\)$/);
  if (!m) throw new Error(`not an opaque oklch() colour: ${value}`);
  const [L, C, h] = [Number(m[1]), Number(m[2]), (Number(m[3]) * Math.PI) / 180];
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const mm = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const clamp = (x: number) => Math.min(1, Math.max(0, x));
  const r = clamp(4.0767416621 * l - 3.3077115913 * mm + 0.2309699292 * s);
  const g = clamp(-1.2684380046 * l + 2.6097574011 * mm - 0.3413193965 * s);
  const bl = clamp(-0.0041960863 * l - 0.7034186147 * mm + 1.707614701 * s);
  return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
}

function contrast(x: string, y: string): number {
  const [hi, lo] = [luminance(x), luminance(y)].sort((p, q) => q - p) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

describe.each([
  ["light", ":root"],
  ["dark", ".dark"],
])("%s theme", (_, selector) => {
  const t = tokens(selector);

  it.each(PAIRS)("%s on %s is at least 4.5:1", (text, surface) => {
    const fg = t.get(text);
    const bg = t.get(surface);
    expect(fg, `--${text} missing`).toBeDefined();
    expect(bg, `--${surface} missing`).toBeDefined();
    expect(contrast(fg ?? "", bg ?? "")).toBeGreaterThanOrEqual(4.5);
  });
});

describe("token lists", () => {
  it("light and dark define the same tokens", () => {
    expect([...tokens(".dark").keys()].sort()).toEqual([...tokens(":root").keys()].sort());
  });

  it("components write no raw colours", async () => {
    const { globSync } = await import("node:fs");
    const files = globSync("src/components/**/*.tsx", { cwd: resolve(__dirname, "../..") });
    const offenders = files.filter((f) => {
      const text = readFileSync(resolve(__dirname, "../..", f), "utf8");
      return /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|hsl|oklch)\(/.test(text);
    });
    expect(offenders).toEqual([]);
  });
});
