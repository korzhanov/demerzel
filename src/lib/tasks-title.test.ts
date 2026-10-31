import { describe, expect, test } from "bun:test";
import { cleanTaskTitle } from "./tasks";

describe("cleanTaskTitle", () => {
  test("keeps the number and text from a markdown link", () => {
    expect(cleanTaskTitle("[#35](https://github.com/korzhanov/100words/issues/35) Квоты A1–C2")).toBe(
      "#35 Квоты A1–C2",
    );
  });
  test("drops bare urls", () => {
    expect(cleanTaskTitle("Починить https://example.com/x")).toBe("Починить");
  });
  test("leaves plain titles alone", () => {
    expect(cleanTaskTitle("Dashboard: Agent CLI")).toBe("Dashboard: Agent CLI");
  });
});
