import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { SYSTEM_PROMPT } from "./agent.js";
import { TOOL_DEFINITIONS } from "./tools.js";
import { readAllSessions } from "./traces.js";
import type { ParsedSession } from "./types.js";

interface ToolMessage {
  role: "assistant";
  tool_calls: Array<{
    id: string;
    type: "function";
    function: { name: string; arguments: string };
  }>;
}

type TrainingMessage =
  | { role: "system" | "user" | "assistant" | "tool"; content: string; tool_call_id?: string }
  | ToolMessage;

interface TrainingExample {
  messages: TrainingMessage[];
  tools: typeof TOOL_DEFINITIONS;
}

function latestRating(session: ParsedSession): "good" | "bad" | undefined {
  return session.events
    .filter(event => event.kind === "rating")
    .at(-1)?.rating;
}

function isSuccessfulAcceptedSession(session: ParsedSession): boolean {
  const requests = session.events.filter(event => event.kind === "tool_request");
  const decisions = session.events.filter(event => event.kind === "tool_decision");
  return (
    session.events.some(event => event.kind === "completed") &&
    !session.events.some(event => event.kind === "failed") &&
    decisions.every(event => event.allowed) &&
    (requests.length === 0 || decisions.length >= requests.length)
  );
}

function splitFor(session: ParsedSession): "train" | "valid" | "test" {
  const identity = session.header.taskId ?? session.header.sessionId;
  const bucket = createHash("sha256").update(identity).digest()[0] ?? 0;
  const percentile = (bucket / 256) * 100;
  if (percentile < 80) return "train";
  if (percentile < 90) return "valid";
  return "test";
}

function transform(session: ParsedSession): TrainingExample | undefined {
  const messages: TrainingMessage[] = [
    { role: "system", content: SYSTEM_PROMPT },
  ];
  const pending = new Map<number, string>();

  for (const event of session.events) {
    if (event.kind === "user" || event.kind === "assistant") {
      if (event.content.trim()) {
        messages.push({ role: event.kind, content: event.content });
      }
    } else if (event.kind === "tool_request") {
      const id = "call_" + String(event.callId);
      pending.set(event.callId, id);
      messages.push({
        role: "assistant",
        tool_calls: [
          {
            id,
            type: "function",
            function: {
              name: event.name,
              arguments: JSON.stringify(event.arguments),
            },
          },
        ],
      });
    } else if (event.kind === "tool_result") {
      const id = pending.get(event.callId);
      if (id) {
        messages.push({
          role: "tool",
          tool_call_id: id,
          content: event.content,
        });
      }
    }
  }

  const hasUser = messages.some(message => message.role === "user");
  const hasAssistant = messages.some(message => message.role === "assistant");
  return hasUser && hasAssistant
    ? { messages, tools: TOOL_DEFINITIONS }
    : undefined;
}

export interface ExportReport {
  totalSessions: number;
  positivelyRatedSessions: number;
  eligibleSessions: number;
  skippedMalformedSessions: number;
  train: number;
  valid: number;
  test: number;
}

export async function exportTrainingData(options: {
  dataDirectory: string;
  outputDirectory: string;
  minimumSessions?: number;
}): Promise<ExportReport> {
  const sessions = await readAllSessions(options.dataDirectory);
  const good = sessions.filter(session => latestRating(session) === "good");
  const eligible = good.filter(isSuccessfulAcceptedSession);
  const minimum = options.minimumSessions ?? 200;
  if (eligible.length < minimum) {
    throw new Error(
      "Need at least " +
        String(minimum) +
        " positively rated, successful sessions; found " +
        String(eligible.length) +
        ".",
    );
  }

  const output: Record<"train" | "valid" | "test", TrainingExample[]> = {
    train: [],
    valid: [],
    test: [],
  };
  let skippedMalformedSessions = 0;

  // Split raw sessions before transforming them to prevent task leakage.
  for (const session of eligible) {
    const split = splitFor(session);
    const example = transform(session);
    if (example) output[split].push(example);
    else skippedMalformedSessions += 1;
  }

  await mkdir(options.outputDirectory, { recursive: true });
  for (const split of ["train", "valid", "test"] as const) {
    const body = output[split].map(value => JSON.stringify(value)).join("\n");
    await writeFile(
      join(options.outputDirectory, split + ".jsonl"),
      body ? body + "\n" : "",
      "utf8",
    );
  }

  const report: ExportReport = {
    totalSessions: sessions.length,
    positivelyRatedSessions: good.length,
    eligibleSessions: eligible.length,
    skippedMalformedSessions,
    train: output.train.length,
    valid: output.valid.length,
    test: output.test.length,
  };
  await writeFile(
    join(options.outputDirectory, "export-report.json"),
    JSON.stringify(report, null, 2) + "\n",
    "utf8",
  );
  return report;
}
