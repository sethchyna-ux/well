import { randomUUID } from "node:crypto";
import { appendFile, mkdir, readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { sanitize } from "./redact.js";
import type {
  ParsedSession,
  Rating,
  TraceEvent,
  TraceHeader,
} from "./types.js";

function cleanValue(value: unknown, maxBytes: number): unknown {
  if (typeof value === "string") {
    return sanitize(value, maxBytes);
  }
  if (Array.isArray(value)) {
    return value.map(item => cleanValue(item, maxBytes));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [
        key,
        cleanValue(item, maxBytes),
      ]),
    );
  }
  return value;
}

export class TraceStore {
  readonly sessionId: string;
  readonly filePath: string;

  private constructor(
    private readonly sessionDirectory: string,
    private readonly maxBytes: number,
    sessionId: string,
  ) {
    this.sessionId = sessionId;
    this.filePath = join(sessionDirectory, sessionId + ".jsonl");
  }

  static async create(
    dataDirectory: string,
    maxBytes: number,
    header: Omit<TraceHeader, "kind" | "schemaVersion" | "sessionId">,
  ): Promise<TraceStore> {
    const sessionDirectory = join(dataDirectory, "sessions");
    await mkdir(sessionDirectory, { recursive: true, mode: 0o700 });
    const store = new TraceStore(sessionDirectory, maxBytes, randomUUID());
    await store.append({
      kind: "session",
      schemaVersion: 1,
      sessionId: store.sessionId,
      ...header,
    });
    return store;
  }

  async event(event: TraceEvent): Promise<void> {
    await this.append(event);
  }

  private async append(value: TraceHeader | TraceEvent): Promise<void> {
    const clean = cleanValue(value, this.maxBytes);
    await appendFile(this.filePath, JSON.stringify(clean) + "\n", {
      encoding: "utf8",
      mode: 0o600,
    });
  }
}

export async function rateSession(
  dataDirectory: string,
  sessionId: string,
  rating: Rating,
): Promise<void> {
  if (!/^[0-9a-f-]{36}$/i.test(sessionId)) {
    throw new Error("Invalid session ID.");
  }
  const path = join(dataDirectory, "sessions", sessionId + ".jsonl");
  await readSession(path);
  await appendFile(
    path,
    JSON.stringify({ kind: "rating", at: new Date().toISOString(), rating }) +
      "\n",
    "utf8",
  );
}

export async function readSession(path: string): Promise<ParsedSession> {
  const lines = (await readFile(path, "utf8"))
    .split("\n")
    .filter(line => line.trim().length > 0)
    .map(line => JSON.parse(line) as TraceHeader | TraceEvent);
  const [header, ...events] = lines;
  if (!header || header.kind !== "session" || header.schemaVersion !== 1) {
    throw new Error("Invalid session trace: " + path);
  }
  return { header, events: events as TraceEvent[] };
}

export async function readAllSessions(
  dataDirectory: string,
): Promise<ParsedSession[]> {
  const directory = join(dataDirectory, "sessions");
  let files: string[];
  try {
    files = await readdir(directory);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }

  const sessions = await Promise.all(
    files
      .filter(file => file.endsWith(".jsonl"))
      .sort()
      .map(file => readSession(join(directory, file))),
  );
  return sessions;
}
