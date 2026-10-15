import assert from "node:assert/strict";
import test from "node:test";
import { redactSecrets, truncateUtf8 } from "../src/redact.js";

test("redacts common secret formats", () => {
  const input =
    "token=super-secret-value Bearer abcdefghijklmnopqrstuvwxyz " +
    "hf_abcdefghijklmnopqrstuvwxyz";
  const output = redactSecrets(input);
  assert.doesNotMatch(output, /super-secret-value/);
  assert.doesNotMatch(output, /abcdefghijklmnopqrstuvwxyz/);
  assert.match(output, /REDACTED/);
});

test("truncates tool output by UTF-8 byte count", () => {
  const output = truncateUtf8("x".repeat(200), 64);
  assert.ok(Buffer.byteLength(output) <= 64);
  assert.match(output, /TRUNCATED/);
});
