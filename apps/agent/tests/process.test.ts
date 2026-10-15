import assert from "node:assert/strict";
import test from "node:test";
import { runProcess } from "../src/process.js";

test("process runner caps output", async () => {
  const result = await runProcess(
    process.execPath,
    ["-e", "process.stdout.write('x'.repeat(10000))"],
    { cwd: process.cwd(), timeoutMs: 5_000, maxOutputBytes: 128 },
  );
  assert.equal(result.exitCode, 0);
  assert.equal(Buffer.byteLength(result.stdout), 128);
});

test("process runner terminates commands at the configured timeout", async () => {
  const result = await runProcess(
    process.execPath,
    ["-e", "setTimeout(() => {}, 10000)"],
    { cwd: process.cwd(), timeoutMs: 50, maxOutputBytes: 128 },
  );
  assert.equal(result.timedOut, true);
});

test("process runner removes secret-bearing environment variables", async () => {
  process.env.WELL_AGENT_TEST_TOKEN = "do-not-expose";
  try {
    const result = await runProcess(
      process.execPath,
      ["-e", "process.stdout.write(process.env.WELL_AGENT_TEST_TOKEN ?? 'absent')"],
      { cwd: process.cwd(), timeoutMs: 5_000, maxOutputBytes: 128 },
    );
    assert.equal(result.stdout, "absent");
  } finally {
    delete process.env.WELL_AGENT_TEST_TOKEN;
  }
});
