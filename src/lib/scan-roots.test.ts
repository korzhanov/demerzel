import { describe, expect, test } from "bun:test";
import {
  allEnabled,
  disabledPaths,
  setSubtreeEnabled,
  setTopLevelEnabled,
  topLevelEntries,
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

describe("вложенные папки не попадают в корень", () => {
  const root = { path: "/work/shop", enabled: true, rules: null };

  test("отключение подпапки не создаёт корневых записей", () => {
    const next = setSubtreeEnabled([root], "/work/shop/www", false);
    expect(topLevelEntries(next).map((e) => e.path)).toEqual(["/work/shop"]);
    expect(enabledRootPaths(next)).toEqual(["/work/shop"]);
    expect(disabledPaths(next)).toEqual(["/work/shop/www"]);
  });

  test("каскад папки убирает переопределения потомков", () => {
    let entries = setSubtreeEnabled([root], "/work/shop/www", false);
    entries = setSubtreeEnabled(entries, "/work/shop", false);
    expect(entries.map((e) => e.path)).toEqual(["/work/shop"]);
    expect(enabledRootPaths(entries)).toEqual([]);
  });

  test("подпапка с таким же именем в другой ветке не задевается", () => {
    const entries = [
      { path: "/work/shop", enabled: true, rules: null },
      { path: "/work/blog", enabled: true, rules: null },
    ];
    const next = setSubtreeEnabled(entries, "/work/shop/www", false);
    expect(next.find((e) => e.path === "/work/blog")?.enabled).toBe(true);
  });

  test("выбор всех верхних папок каскадом", () => {
    const entries = [root, { path: "/work/blog", enabled: true, rules: null }];
    const off = setTopLevelEnabled(entries, false);
    expect(enabledRootPaths(off)).toEqual([]);
    expect(enabledRootPaths(setTopLevelEnabled(off, true))).toEqual(["/work/shop", "/work/blog"]);
  });
});
