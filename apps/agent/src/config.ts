import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { z } from "zod";
import type { AgentConfig } from "./types.js";

const configSchema = z
  .object({
    model: z.string().min(1).optional(),
    maxPredictionRounds: z.number().int().min(1).max(32).optional(),
    commandTimeoutMs: z.number().int().min(1_000).max(300_000).optional(),
    maxToolOutputBytes: z.number().int().min(1_024).max(1_048_576).optional(),
    recordSessions: z.boolean().optional(),
    dataDirectory: z.string().min(1).optional(),
  })
  .strict();

export function defaultDataDirectory(): string {
  return (
    process.env.WELL_AGENT_HOME ??
    join(homedir(), "Library", "Application Support", "well-agent")
  );
}

export const DEFAULT_CONFIG: AgentConfig = {
  model: "qwen/qwen3-4b-2507",
  maxPredictionRounds: 8,
  commandTimeoutMs: 30_000,
  maxToolOutputBytes: 64 * 1024,
  recordSessions: true,
  dataDirectory: defaultDataDirectory(),
};

export async function loadConfig(configPath?: string): Promise<AgentConfig> {
  const path = configPath
    ? resolve(configPath)
    : join(defaultDataDirectory(), "config.json");

  let parsed: unknown;
  try {
    parsed = JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return { ...DEFAULT_CONFIG };
    }
    throw new Error("Unable to read agent configuration at " + path, {
      cause: error,
    });
  }

  const overrides = configSchema.parse(parsed);
  return {
    ...DEFAULT_CONFIG,
    ...overrides,
    dataDirectory: resolve(
      overrides.dataDirectory ?? DEFAULT_CONFIG.dataDirectory,
    ),
  };
}
