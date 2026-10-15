import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { exportTrainingData } from "../src/exporter.js";
import { rateSession, readSession, TraceStore } from "../src/traces.js";

test("records, rates, and exports a tool-use session", async () => {
  const dataDirectory = await mkdtemp(join(tmpdir(), "well-traces-"));
  const outputDirectory = join(dataDirectory, "export");
  const trace = await TraceStore.create(dataDirectory, 64 * 1024, {
    taskId: "test-task",
    model: "test-model",
    workspace: "/tmp/workspace",
    startedAt: new Date().toISOString(),
  });
  await trace.event({
    kind: "user",
    at: new Date().toISOString(),
    content: "Read README.md",
  });
  await trace.event({
    kind: "tool_request",
    at: new Date().toISOString(),
    callId: 1,
    name: "read_file",
    arguments: { path: "README.md" },
  });
  await trace.event({
    kind: "tool_result",
    at: new Date().toISOString(),
    callId: 1,
    name: "read_file",
    content: "# Example",
  });
  await trace.event({
    kind: "tool_decision",
    at: new Date().toISOString(),
    callId: 1,
    allowed: true,
  });
  await trace.event({
    kind: "assistant",
    at: new Date().toISOString(),
    content: "The title is Example.",
  });
  await trace.event({
    kind: "completed",
    at: new Date().toISOString(),
    durationMs: 10,
  });
  await rateSession(dataDirectory, trace.sessionId, "good");

  const parsed = await readSession(trace.filePath);
  assert.equal(parsed.header.taskId, "test-task");
  assert.equal(parsed.events.at(-1)?.kind, "rating");

  const report = await exportTrainingData({
    dataDirectory,
    outputDirectory,
    minimumSessions: 1,
  });
  assert.equal(report.positivelyRatedSessions, 1);
  assert.equal(report.train + report.valid + report.test, 1);
  const contents = await Promise.all(
    ["train", "valid", "test"].map(name =>
      readFile(join(outputDirectory, name + ".jsonl"), "utf8"),
    ),
  );
  const exported = contents.join("");
  assert.match(exported, /read_file/);
  assert.match(exported, /tool_calls/);
});
