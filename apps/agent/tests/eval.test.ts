import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { loadEvalTasks } from "../src/eval.js";

test("evaluation suite contains 30 balanced, unique tasks", async () => {
  const suite = fileURLToPath(new URL("../evals/tasks.jsonl", import.meta.url));
  const tasks = await loadEvalTasks(suite);
  assert.equal(tasks.length, 30);
  assert.equal(new Set(tasks.map(task => task.id)).size, 30);
  assert.equal(tasks.filter(task => task.category === "navigation").length, 10);
  assert.equal(tasks.filter(task => task.category === "diagnosis").length, 10);
  assert.equal(tasks.filter(task => task.category === "patch").length, 10);
  assert.ok(
    tasks
      .filter(task => task.category === "patch")
      .every(task => task.allowMutations),
  );
});
