#!/usr/bin/env bun
/**
 * Sync korzhanov/demerzel issues + PRs into user project #13 with Status.
 *
 * Mapping:
 *   open PR (ready)  → In review
 *   draft PR         → In progress
 *   merged/closed PR → Done
 *   open issue       → Backlog
 *   closed issue     → Done
 *   open issue mentioned by an open PR → In progress
 *
 * Requires: gh auth with `project` + `repo` scopes.
 */
import { $ } from "bun";

const OWNER = "korzhanov";
const REPO = "korzhanov/demerzel";
const PROJECT_NUMBER = 13;
const PROJECT_ID = "PVT_kwHOACIdh84BpdKI";
const STATUS_FIELD_ID = "PVTSSF_lAHOACIdh84BpdKIzhn4vMQ";
const STATUS = {
  Backlog: "f75ad846",
  Ready: "61e4505c",
  "In progress": "47fc9ee4",
  "In review": "df73e18b",
  Done: "98236657",
};

async function ghText(args) {
  const r = await $`gh ${args}`.quiet().nothrow();
  if (r.exitCode !== 0) {
    throw new Error(`gh ${args.join(" ")} failed:\n${r.stderr.toString()}`);
  }
  return r.stdout.toString().trim();
}

async function ghJson(args) {
  const text = await ghText(args);
  return text ? JSON.parse(text) : null;
}

async function contentNodeId(url) {
  // https://github.com/owner/repo/issues/N or /pull/N
  const m = url.match(/github\.com\/([^/]+)\/([^/]+)\/(issues|pull)\/(\d+)/);
  if (!m) throw new Error(`bad url ${url}`);
  const [, owner, repo, kind, num] = m;
  const path =
    kind === "issues"
      ? `repos/${owner}/${repo}/issues/${num}`
      : `repos/${owner}/${repo}/pulls/${num}`;
  return ghText(["api", path, "--jq", ".node_id"]);
}

async function ensureItem(url) {
  const contentId = await contentNodeId(url);
  const add = await ghJson([
    "api",
    "graphql",
    "-f",
    `query=mutation($project:ID!, $content:ID!) {
      addProjectV2ItemById(input: {projectId: $project, contentId: $content}) {
        item { id }
      }
    }`,
    "-f",
    `project=${PROJECT_ID}`,
    "-f",
    `content=${contentId}`,
  ]);
  let itemId = add?.data?.addProjectV2ItemById?.item?.id;
  if (itemId) return itemId;

  // Already on board — GraphQL returns errors in .errors
  const listed = await ghJson([
    "project",
    "item-list",
    String(PROJECT_NUMBER),
    "--owner",
    OWNER,
    "--format",
    "json",
    "--limit",
    "500",
  ]);
  const match = (listed.items || []).find((it) => it.content?.url === url);
  if (!match) {
    throw new Error(
      `cannot add/find item ${url}: ${JSON.stringify(add?.errors || add)}`,
    );
  }
  return match.id;
}

async function setStatus(itemId, statusName) {
  const optionId = STATUS[statusName];
  if (!optionId) throw new Error(`unknown status ${statusName}`);
  await $`gh project item-edit --id ${itemId} --project-id ${PROJECT_ID} --field-id ${STATUS_FIELD_ID} --single-select-option-id ${optionId}`.quiet();
}

function statusForPr(pr) {
  if (pr.state === "MERGED" || pr.state === "CLOSED") return "Done";
  if (pr.isDraft) return "In progress";
  return "In review";
}

function statusForIssue(issue, openPrIssueNumbers) {
  if (issue.state === "CLOSED") return "Done";
  if (openPrIssueNumbers.has(issue.number)) return "In progress";
  return "Backlog";
}

const issues = await ghJson([
  "issue",
  "list",
  "--repo",
  REPO,
  "--state",
  "all",
  "--limit",
  "200",
  "--json",
  "number,title,state,url",
]);
const prs = await ghJson([
  "pr",
  "list",
  "--repo",
  REPO,
  "--state",
  "all",
  "--limit",
  "200",
  "--json",
  "number,title,state,isDraft,url",
]);

const openPrIssueNumbers = new Set();
for (const pr of prs.filter((p) => p.state === "OPEN")) {
  const full = await ghJson([
    "pr",
    "view",
    String(pr.number),
    "--repo",
    REPO,
    "--json",
    "body,closingIssuesReferences",
  ]);
  for (const ref of full.closingIssuesReferences || []) {
    openPrIssueNumbers.add(ref.number);
  }
  for (const m of (full.body || "").matchAll(/#(\d+)/g)) {
    openPrIssueNumbers.add(Number(m[1]));
  }
}

console.log(`Sync → https://github.com/users/${OWNER}/projects/${PROJECT_NUMBER}`);
console.log(`PRs: ${prs.length}, issues: ${issues.length}`);

for (const pr of prs) {
  const status = statusForPr(pr);
  process.stdout.write(`PR #${pr.number} → ${status} … `);
  const id = await ensureItem(pr.url);
  await setStatus(id, status);
  console.log("ok");
}

for (const issue of issues) {
  const status = statusForIssue(issue, openPrIssueNumbers);
  process.stdout.write(`Issue #${issue.number} → ${status} … `);
  const id = await ensureItem(issue.url);
  await setStatus(id, status);
  console.log("ok");
}

console.log("Done.");
