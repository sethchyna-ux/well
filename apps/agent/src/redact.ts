const SECRET_PATTERNS: Array<[RegExp, string]> = [
  [/-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g, "[REDACTED PRIVATE KEY]"],
  [/\bsk-[A-Za-z0-9_-]{20,}\b/g, "[REDACTED API KEY]"],
  [/\bhf_[A-Za-z0-9]{20,}\b/g, "[REDACTED HF TOKEN]"],
  [/\bAKIA[0-9A-Z]{16}\b/g, "[REDACTED AWS KEY]"],
  [/\bBearer\s+[A-Za-z0-9._~+/=-]{12,}\b/gi, "Bearer [REDACTED]"],
  [/((?:password|passwd|token|api[_-]?key|secret)\s*[:=]\s*)[^\s,;]+/gi, "$1[REDACTED]"],
];

export function redactSecrets(value: string): string {
  return SECRET_PATTERNS.reduce(
    (current, [pattern, replacement]) => current.replace(pattern, replacement),
    value,
  );
}

export function truncateUtf8(value: string, maxBytes: number): string {
  const data = Buffer.from(value);
  if (data.byteLength <= maxBytes) {
    return value;
  }
  return (
    data.subarray(0, Math.max(0, maxBytes - 32)).toString("utf8") +
    "\n[OUTPUT TRUNCATED]"
  );
}

export function sanitize(value: string, maxBytes: number): string {
  return truncateUtf8(redactSecrets(value), maxBytes);
}
