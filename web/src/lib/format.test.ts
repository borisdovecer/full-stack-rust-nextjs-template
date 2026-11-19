import { describe, expect, it } from "vitest";

import { formatDate, formatName, formatNumber, formatRelative } from "./format";

describe("format", () => {
  it("writes the full name as first then last", () => {
    expect(formatName({ first_name: "Ana", last_name: "Example" })).toBe("Ana Example");
  });

  it("shows a dash for missing or broken values", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate("not a date")).toBe("—");
    expect(formatNumber(undefined)).toBe("—");
    expect(formatRelative(undefined)).toBe("—");
  });

  it("writes relative times in the largest whole unit", () => {
    const now = new Date("2030-01-10T12:00:00Z");
    expect(formatRelative("2030-01-10T09:00:00Z", now)).toMatch(/3 hours ago/);
    expect(formatRelative("2030-01-12T12:00:00Z", now)).toMatch(/in 2 days/);
    expect(formatRelative("2030-01-10T11:59:50Z", now)).toMatch(/now/);
  });

  it("formats numbers with grouping", () => {
    expect(formatNumber(1234567)).toMatch(/1.234.567/);
  });
});
