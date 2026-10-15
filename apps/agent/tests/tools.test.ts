import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import test from "node:test";
import { DEFAULT_CONFIG } from "../src/config.js";
import { WorkspaceBoundary } from "../src/paths.js";
import { createAgentTools } from "../src/tools.js";

const execFileAsync = promisify(execFile);

test("read and patch tools stay inside the workspace", async () => {
  const workspace = await mkdtemp(join(tmpdir(), "well-tools-"));
  await execFileAsync("git", ["init"], { cwd: workspace });
  await writeFile(join(workspace, "README.md"), "# Before\n");
  const boundary = await WorkspaceBoundary.create(workspace);
  const tools = await createAgentTools(boundary, {
    ...DEFAULT_CONFIG,
    recordSessions: false,
  });
  const readTool = tools.find(tool => tool.name === "read_file") as any;
  const patchTool = tools.find(tool => tool.name === "apply_patch") as any;

  const readResult = await readTool.implementation({ path: "README.md" }, {});
  assert.match(readResult, /Before/);

  const patch = [
    "diff --git a/README.md b/README.md",
    "--- a/README.md",
    "+++ b/README.md",
    "@@ -1 +1 @@",
    "-# Before",
    "+# After",
    "",
  ].join("\n");
  const patchResult = await patchTool.implementation({ patch }, {});
  assert.match(patchResult, /exit code 0/);
  assert.equal(await readFile(join(workspace, "README.md"), "utf8"), "# After\n");

  await assert.rejects(
    readTool.implementation({ path: "../outside" }, {}),
    /traversal|escapes/,
  );

  const outside = await mkdtemp(join(tmpdir(), "well-tools-outside-"));
  await writeFile(join(outside, "outside.md"), "# Outside\n");
  await symlink(join(outside, "outside.md"), join(workspace, "linked.md"));
  const escapePatch = [
    "diff --git a/linked.md b/linked.md",
    "--- a/linked.md",
    "+++ b/linked.md",
    "@@ -1 +1 @@",
    "-# Outside",
    "+# Escaped",
    "",
  ].join("\n");
  await assert.rejects(
    patchTool.implementation({ patch: escapePatch }, {}),
    /escapes/,
  );
});
