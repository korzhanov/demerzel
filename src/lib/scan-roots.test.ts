import { describe, expect, test } from "bun:test";
import {
  allEnabled,
  enabledRootPaths,
  entriesFromLegacyRoots,
  hasCustomRules,
  isPathUnder,
  noneEnabled,
  normalizePath,
  removeEntry,
  resolveEntryForPath,
  resolveRulesForPath,
  setAllEnabled,
  upsertEntry,
  type ScanRootEntry,
} from "./scan-roots";

describe("scan-roots model", () => {
  test("normalizePath strips trailing slash", () => {
    expect(normalizePath("/a/b/")).toBe("/a/b");
    expect(normalizePath("  /x  ")).toBe("/x");
  });

  test("isPathUnder", () => {
    expect(isPathUnder("/code/app", "/code")).toBe(true);
    expect(isPathUnder("/code", "/code")).toBe(true);
    expect(isPathUnder("/code2", "/code")).toBe(false);
    expect(isPathUnder("/co", "/code")).toBe(false);
  });

  test("nested entry wins rules priority", () => {
    const entries: ScanRootEntry[] = [
      { path: "/code", enabled: true, rules: { max_depth: 2 } },
      { path: "/code/app", enabled: true, rules: { max_depth: 5, ignore: ["tmp"] } },
    ];
    expect(resolveRulesForPath("/code/app/src", entries)?.max_depth).toBe(5);
    expect(resolveRulesForPath("/code/app/src", entries)?.ignore).toEqual(["tmp"]);
    expect(resolveRulesForPath("/code/other", entries)?.max_depth).toBe(2);
    expect(resolveEntryForPath("/code/app/x", entries)?.path).toBe("/code/app");
  });

  test("legacy roots → entries; enabled paths mirror", () => {
    const entries = entriesFromLegacyRoots([" /a ", "/a", "/b"]);
    expect(entries).toEqual([
      { path: "/a", enabled: true, rules: null },
      { path: "/b", enabled: true, rules: null },
    ]);
    expect(enabledRootPaths(setAllEnabled(entries, false))).toEqual([]);
    expect(allEnabled(entries)).toBe(true);
    expect(noneEnabled(setAllEnabled(entries, false))).toBe(true);
  });

  test("upsert / remove / hasCustomRules", () => {
    let entries: ScanRootEntry[] = [];
    entries = upsertEntry(entries, "/work");
    entries = upsertEntry(entries, "/work", { rules: { max_nesting: 0 } });
    expect(entries).toHaveLength(1);
    expect(hasCustomRules(entries[0])).toBe(true);
    entries = removeEntry(entries, "/work/");
    expect(entries).toHaveLength(0);
  });
});
