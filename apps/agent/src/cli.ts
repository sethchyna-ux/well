#!/usr/bin/env node
import { createInterface } from "node:readline/promises";
import { stdin, stdout } from "node:process";
import { resolve } from "node:path";
import { AgentRuntime } from "./agent.js";
import { loadConfig } from "./config.js";
import { runEvaluation } from "./eval.js";
import { exportTrainingData } from "./exporter.js";
import { rateSession } from "./traces.js";
import type { EvalTask, Rating } from "./types.js";

interface ParsedArguments {
  positionals: string[];
  options: Map<string, string>;
  flags: Set<string>;
}

function parseArguments(args: string[]): ParsedArguments {
  const parsed: ParsedArguments = {
    positionals: [],
    options: new Map(),
    flags: new Set(),
  };
  for (let index = 0; index < args.length; index += 1) {
    const current = args[index]!;
    if (!current.startsWith("--")) {
      parsed.positionals.push(current);
      continue;
    }
    const name = current.slice(2);
    const next = args[index + 1];
    if (next !== undefined && !next.startsWith("--")) {
      parsed.options.set(name, next);
      index += 1;
    } else {
      parsed.flags.add(name);
    }
  }
  return parsed;
}

function requiredOption(args: ParsedArguments, name: string): string {
  const value = args.options.get(name);
  if (!value) throw new Error("Missing required option --" + name);
  return value;
}

function usage(): string {
  return [
    "Usage:",
    "  well-agent chat --workspace PATH [--model ID] [--config PATH]",
    "  well-agent eval --suite FILE [--task ID] [--category navigation|diagnosis|patch] [--model ID] [--output FILE]",
    "  well-agent export-data --output DIR [--min-sessions NUMBER]",
    "  well-agent rate SESSION_ID good|bad",
  ].join("\n");
}

async function chat(args: ParsedArguments): Promise<void> {
  const config = await loadConfig(args.options.get("config"));
  if (args.options.has("model")) config.model = args.options.get("model")!;
  const workspace = resolve(requiredOption(args, "workspace"));
  const rl = createInterface({ input: stdin, output: stdout });

  const runtime = await AgentRuntime.create(workspace, config, {
    taskId: args.options.get("task-id"),
    onText: text => stdout.write(text),
    approval: async (name, argumentsValue) => {
      stdout.write(
        "\nApproval required for " +
          name +
          ":\n" +
          JSON.stringify(argumentsValue, null, 2) +
          "\n",
      );
      const answer = await rl.question("Allow this tool call? [y/N] ");
      return answer.trim().toLowerCase() === "y";
    },
  });

  stdout.write(
    "Well Agent\nWorkspace: " +
      workspace +
      "\nModel: " +
      config.model +
      "\nSession: " +
      runtime.sessionId +
      "\nType /exit to stop.\n\n",
  );

  try {
    for (;;) {
      const input = await rl.question("You: ");
      if (input.trim() === "/exit") break;
      if (!input.trim()) continue;
      stdout.write("Agent: ");
      const result = await runtime.runOnce(input);
      if (!result.response) stdout.write("[No textual response]");
      stdout.write("\n\n");
    }
  } finally {
    rl.close();
  }
}

async function main(): Promise<void> {
  const [command, ...rest] = process.argv.slice(2);
  const args = parseArguments(rest);
  if (!command || command === "help" || args.flags.has("help")) {
    stdout.write(usage() + "\n");
    return;
  }

  if (command === "chat") {
    await chat(args);
    return;
  }

  const config = await loadConfig(args.options.get("config"));
  if (args.options.has("model")) config.model = args.options.get("model")!;

  if (command === "rate") {
    const [sessionId, rawRating] = args.positionals;
    if (!sessionId || (rawRating !== "good" && rawRating !== "bad")) {
      throw new Error("Usage: well-agent rate SESSION_ID good|bad");
    }
    await rateSession(config.dataDirectory, sessionId, rawRating as Rating);
    stdout.write("Rated session " + sessionId + " as " + rawRating + ".\n");
    return;
  }

  if (command === "export-data") {
    const outputDirectory = resolve(requiredOption(args, "output"));
    const minimumSessions = Number(args.options.get("min-sessions") ?? "200");
    if (!Number.isInteger(minimumSessions) || minimumSessions < 1) {
      throw new Error("--min-sessions must be a positive integer.");
    }
    const report = await exportTrainingData({
      dataDirectory: config.dataDirectory,
      outputDirectory,
      minimumSessions,
    });
    stdout.write(JSON.stringify(report, null, 2) + "\n");
    return;
  }

  if (command === "eval") {
    const suitePath = resolve(requiredOption(args, "suite"));
    const category = args.options.get("category");
    if (
      category !== undefined &&
      category !== "navigation" &&
      category !== "diagnosis" &&
      category !== "patch"
    ) {
      throw new Error("--category must be navigation, diagnosis, or patch.");
    }
    const summary = await runEvaluation({
      suitePath,
      config: { ...config, recordSessions: false },
      taskId: args.options.get("task"),
      category: category as EvalTask["category"] | undefined,
      outputPath: args.options.get("output")
        ? resolve(args.options.get("output")!)
        : undefined,
      onTaskComplete: (result, completed, total) => {
        const status = result.passed ? "pass" : "fail";
        console.error(
          `[${completed}/${total}] ${status} ${result.id} (${result.durationMs}ms)`,
        );
      },
    });
    stdout.write(JSON.stringify(summary, null, 2) + "\n");
    process.exitCode = summary.successRate >= 0.7 ? 0 : 1;
    return;
  }

  throw new Error("Unknown command: " + command + "\n" + usage());
}

main().catch(error => {
  process.stderr.write(
    "well-agent: " +
      (error instanceof Error ? error.message : String(error)) +
      "\n",
  );
  process.exitCode = 1;
});
