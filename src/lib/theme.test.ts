import { describe, expect, test } from "bun:test";
import { parseThemeValue } from "./theme";

describe("theme", () => {
  test("parseThemeValue", () => {
    expect(parseThemeValue(null)).toBe("dark");
    expect(parseThemeValue(undefined)).toBe("dark");
    expect(parseThemeValue("dark")).toBe("dark");
    expect(parseThemeValue("light")).toBe("light");
    expect(parseThemeValue("nope")).toBe("dark");
  });
});
