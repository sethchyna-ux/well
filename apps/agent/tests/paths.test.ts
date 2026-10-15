import assert from "node:assert/strict";
import { mkdtemp, mkdir, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { WorkspaceBoundary, validateRelativePath } from "../src/paths.js";

test("workspace boundary accepts contained paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "well-paths-"));
  await mkdir(join(root, "src"));
  await writeFile(join(root, "src", "main.ts"), "export {};\n");
  const boundary = await WorkspaceBoundary.create(root);

  assert.equal(
    await boundary.existing("src/main.ts"),
    join(boundary.root, "src", "main.ts"),
  );
  assert.equal(
    await boundary.prospective("src/new.ts"),
    join(boundary.root, "src", "new.ts"),
  );
});

test("workspace boundary rejects traversal and symlink escapes", async () => {
  const root = await mkdtemp(join(tmpdir(), "well-paths-"));
  const outside = await mkdtemp(join(tmpdir(), "well-outside-"));
  await writeFile(join(outside, "secret.txt"), "secret\n");
  await symlink(outside, join(root, "escape"));
  const boundary = await WorkspaceBoundary.create(root);

  await assert.rejects(boundary.existing("../secret.txt"), /escapes/);
  await assert.rejects(boundary.existing("escape/secret.txt"), /escapes/);
  assert.throws(() => validateRelativePath("../file"), /traversal/);
  assert.throws(() => validateRelativePath("/tmp/file"), /relative/);
});
