import assert from "node:assert/strict";
import test from "node:test";
import { authorizeToolCall } from "../src/agent.js";

test("read-only tools bypass approval while mutating tools require it", async () => {
  let approvalCalls = 0;
  const approval = async () => {
    approvalCalls += 1;
    return false;
  };

  assert.equal(await authorizeToolCall("read_file", {}, approval), true);
  assert.equal(approvalCalls, 0);
  assert.equal(await authorizeToolCall("apply_patch", {}, approval), false);
  assert.equal(approvalCalls, 1);
  assert.equal(await authorizeToolCall("run_command", {}, async () => true), true);
});
