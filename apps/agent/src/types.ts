export type Rating = "good" | "bad";

export interface AgentConfig {
  model: string;
  maxPredictionRounds: number;
  commandTimeoutMs: number;
  maxToolOutputBytes: number;
  recordSessions: boolean;
  dataDirectory: string;
}

export interface TraceHeader {
  kind: "session";
  schemaVersion: 1;
  sessionId: string;
  taskId?: string;
  model: string;
  workspace: string;
  startedAt: string;
}

export type TraceEvent =
  | { kind: "user"; at: string; content: string }
  | { kind: "assistant"; at: string; content: string }
  | {
      kind: "tool_request";
      at: string;
      callId: number;
      name: string;
      arguments: Record<string, unknown>;
    }
  | {
      kind: "tool_decision";
      at: string;
      callId: number;
      allowed: boolean;
      reason?: string;
    }
  | {
      kind: "tool_result";
      at: string;
      callId: number;
      name?: string;
      content: string;
    }
  | { kind: "completed"; at: string; durationMs: number }
  | { kind: "failed"; at: string; message: string }
  | { kind: "rating"; at: string; rating: Rating };

export interface ParsedSession {
  header: TraceHeader;
  events: TraceEvent[];
}

export type EvalAssertion =
  | { kind: "response_contains"; value: string }
  | { kind: "file_contains"; path: string; value: string }
  | { kind: "file_not_contains"; path: string; value: string }
  | { kind: "path_exists"; path: string }
  | { kind: "path_missing"; path: string };

export interface EvalTask {
  id: string;
  category: "navigation" | "diagnosis" | "patch";
  fixture: "well" | "pxe" | "rustcubed";
  prompt: string;
  allowMutations: boolean;
  assertions: EvalAssertion[];
}

export interface AgentRunResult {
  response: string;
  durationMs: number;
  sessionId: string;
  toolCallCount: number;
  invalidToolCallCount: number;
  toolCalls: AgentToolCallRecord[];
}

export interface AgentToolCallRecord {
  callId: number;
  name: string;
  arguments: Record<string, unknown>;
  allowed: boolean;
  result?: string;
}
