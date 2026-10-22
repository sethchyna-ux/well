import { cp, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { AgentRuntime } from "./agent.js";
import { validateRelativePath } from "./paths.js";
import { runProcess } from "./process.js";
import type {
  AgentConfig,
  AgentRunResult,
  EvalAssertion,
  EvalTask,
} from "./types.js";

const assertionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("response_contains"), value: z.string() }),
  z.object({
    kind: z.literal("file_contains"),
    path: z.string(),
    value: z.string(),
  }),
  z.object({
    kind: z.literal("file_not_contains"),
    path: z.string(),
    value: z.string(),
  }),
  z.object({ kind: z.literal("path_exists"), path: z.string() }),
  z.object({ kind: z.literal("path_missing"), path: z.string() }),
]);

const taskSchema = z.object({
  id: z.string().min(1),
  category: z.enum(["navigation", "diagnosis", "patch"]),
  fixture: z.enum(["well", "pxe", "rustcubed"]),
  prompt: z.string().min(1),
  allowMutations: z.boolean(),
  assertions: z.array(assertionSchema).min(1),
});

export async function loadEvalTasks(path: string): Promise<EvalTask[]> {
  const lines = (await readFile(path, "utf8"))
    .split("\n")
    .filter(line => line.trim().length > 0);
  const tasks = lines.map((line, index) => {
    try {
      return taskSchema.parse(JSON.parse(line));
    } catch (error) {
      throw new Error(
        "Invalid evaluation task on line " + String(index + 1),
        { cause: error },
      );
    }
  });
  const ids = new Set(tasks.map(task => task.id));
  if (ids.size !== tasks.length) {
    throw new Error("Evaluation task IDs must be unique.");
  }
  return tasks;
}

async function assertionPasses(
  assertion: EvalAssertion,
  workspace: string,
  response: string,
): Promise<boolean> {
  if (assertion.kind === "response_contains") {
    return response.toLowerCase().includes(assertion.value.toLowerCase());
  }
  validateRelativePath(assertion.path);
  const path = resolve(workspace, assertion.path);
  if (assertion.kind === "path_exists" || assertion.kind === "path_missing") {
    let exists = true;
    try {
      await stat(path);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") exists = false;
      else throw error;
    }
    return assertion.kind === "path_exists" ? exists : !exists;
  }
  const content = await readFile(path, "utf8");
  return assertion.kind === "file_contains"
    ? content.includes(assertion.value)
    : !content.includes(assertion.value);
}

function describeAssertion(assertion: EvalAssertion): string {
  if (assertion.kind === "response_contains") {
    return `response contains ${JSON.stringify(assertion.value)}`;
  }
  if (assertion.kind === "path_exists") {
    return `${assertion.path} exists`;
  }
  if (assertion.kind === "path_missing") {
    return `${assertion.path} is missing`;
  }
  const relation =
    assertion.kind === "file_contains" ? "contains" : "does not contain";
  return `${assertion.path} ${relation} ${JSON.stringify(assertion.value)}`;
}

export interface EvalSummary {
  model: string;
  startedAt: string;
  completedAt: string;
  total: number;
  passed: number;
  successRate: number;
  validToolCallRate: number;
  medianDurationMs: number;
  results: Array<{
    id: string;
    category: EvalTask["category"];
    passed: boolean;
    durationMs: number;
    toolCallCount: number;
    invalidToolCallCount: number;
    response?: string;
    failedAssertions?: string[];
    toolCalls?: AgentRunResult["toolCalls"];
    error?: string;
  }>;
}

export async function runEvaluation(options: {
  suitePath: string;
  config: AgentConfig;
  outputPath?: string;
  taskId?: string;
  category?: EvalTask["category"];
  onTaskComplete?: (
    result: EvalSummary["results"][number],
    completed: number,
    total: number,
  ) => void;
}): Promise<EvalSummary> {
  const suitePath = resolve(options.suitePath);
  const loadedTasks = await loadEvalTasks(suitePath);
  const tasks = loadedTasks.filter(
    task =>
      (!options.taskId || task.id === options.taskId) &&
      (!options.category || task.category === options.category),
  );
  if (tasks.length === 0) {
    const selection = options.taskId ?? options.category ?? "suite";
    throw new Error(`No evaluation tasks matched: ${selection}`);
  }
  const fixtures = join(dirname(suitePath), "fixtures");
  const startedAt = new Date().toISOString();
  const results: EvalSummary["results"] = [];

  for (const task of tasks) {
    const temporaryRoot = await mkdtemp(join(tmpdir(), "well-agent-eval-"));
    const workspace = join(temporaryRoot, "workspace");
    await cp(join(fixtures, task.fixture), workspace, { recursive: true });
    await runProcess("git", ["init", "--quiet"], {
      cwd: workspace,
      timeoutMs: 10_000,
      maxOutputBytes: 8_192,
    });

    try {
      const runtime = await AgentRuntime.create(
        workspace,
        { ...options.config, recordSessions: false },
        {
          taskId: task.id,
          approval: async () => task.allowMutations,
        },
      );
      const run = await runtime.runOnce(task.prompt);
      const checks = await Promise.all(
        task.assertions.map(assertion =>
          assertionPasses(assertion, workspace, run.response),
        ),
      );
      const failedAssertions = task.assertions
        .filter((_assertion, index) => !checks[index])
        .map(describeAssertion);
      const result = {
        id: task.id,
        category: task.category,
        passed: checks.every(Boolean),
        durationMs: run.durationMs,
        toolCallCount: run.toolCallCount,
        invalidToolCallCount: run.invalidToolCallCount,
        response: run.response,
        toolCalls: run.toolCalls,
        ...(failedAssertions.length ? { failedAssertions } : {}),
      };
      results.push(result);
      options.onTaskComplete?.(result, results.length, tasks.length);
    } catch (error) {
      const result = {
        id: task.id,
        category: task.category,
        passed: false,
        durationMs: 0,
        toolCallCount: 0,
        invalidToolCallCount: 1,
        error: error instanceof Error ? error.message : String(error),
      };
      results.push(result);
      options.onTaskComplete?.(result, results.length, tasks.length);
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true });
    }
  }

  const durations = results
    .map(result => result.durationMs)
    .filter(duration => duration > 0)
    .sort((a, b) => a - b);
  const middle = Math.floor(durations.length / 2);
  const medianDurationMs =
    durations.length === 0
      ? 0
      : durations.length % 2
        ? durations[middle] ?? 0
        : ((durations[middle - 1] ?? 0) + (durations[middle] ?? 0)) / 2;
  const passed = results.filter(result => result.passed).length;
  const totalToolCalls = results.reduce(
    (total, result) => total + result.toolCallCount + result.invalidToolCallCount,
    0,
  );
  const invalidToolCalls = results.reduce(
    (total, result) => total + result.invalidToolCallCount,
    0,
  );
  const summary: EvalSummary = {
    model: options.config.model,
    startedAt,
    completedAt: new Date().toISOString(),
    total: tasks.length,
    passed,
    successRate: tasks.length ? passed / tasks.length : 0,
    validToolCallRate: totalToolCalls
      ? (totalToolCalls - invalidToolCalls) / totalToolCalls
      : 1,
    medianDurationMs,
    results,
  };

  const outputPath =
    options.outputPath ??
    join(dirname(suitePath), "results", "latest.json");
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, JSON.stringify(summary, null, 2) + "\n", "utf8");
  return summary;
}
