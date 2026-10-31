import { describe, expect, test } from "bun:test";
import {
  nextTheme,
  parseThemeValue,
  resolveTheme,
  themeLabel,
} from "./theme";

describe("theme", () => {
  test("parseThemeValue", () => {
    expect(parseThemeValue(null)).toBe("dark");
    expect(parseThemeValue(undefined)).toBe("dark");
    expect(parseThemeValue("dark")).toBe("dark");
    expect(parseThemeValue("light")).toBe("light");
    expect(parseThemeValue("system")).toBe("system");
    expect(parseThemeValue("nope")).toBe("dark");
  });

  test("resolveTheme follows system preference", () => {
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  test("nextTheme cycles dark → light → system", () => {
    expect(nextTheme("dark")).toBe("light");
    expect(nextTheme("light")).toBe("system");
    expect(nextTheme("system")).toBe("dark");
  });

  test("themeLabel", () => {
    expect(themeLabel("dark")).toBe("Тёмная тема");
    expect(themeLabel("light")).toBe("Светлая тема");
    expect(themeLabel("system")).toBe("Системная тема");
  });
});
