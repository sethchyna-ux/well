import { randomUUID } from "node:crypto";
import { Chat, LMStudioClient, type Tool } from "@lmstudio/sdk";
import type {
  AgentConfig,
  AgentRunResult,
  AgentToolCallRecord,
} from "./types.js";
import { WorkspaceBoundary } from "./paths.js";
import { READ_ONLY_TOOLS, createAgentTools } from "./tools.js";
import { TraceStore } from "./traces.js";

export const SYSTEM_PROMPT = [
  "You are Well Agent, a careful local coding and systems assistant.",
  "Inspect the whole workspace before proposing changes: begin with list_files without a glob unless the user names an exact file.",
  "Search without a glob when the relevant directory is unknown, and read likely documentation and source files.",
  "After locating a candidate file, read the relevant lines before answering; use exact declarations and documentation instead of inferring names or values from paths.",
  "Before saying information is absent, list all workspace files and perform at least one workspace-wide search.",
  "Use the provided tools for file and command work.",
  "For a focused edit to an existing file, prefer replace_text with text copied exactly from read_file; use apply_patch for larger edits or new files.",
  "Keep changes focused on the user's request.",
  "Never attempt to escape the selected workspace.",
  "Explain the completed result and any verification failures concisely.",
].join(" ");

export type ApprovalHandler = (
  toolName: string,
  argumentsValue: Record<string, unknown>,
) => Promise<boolean>;

export async function authorizeToolCall(
  toolName: string,
  argumentsValue: Record<string, unknown>,
  approval: ApprovalHandler,
): Promise<boolean> {
  return (
    READ_ONLY_TOOLS.has(toolName) ||
    (await approval(toolName, argumentsValue))
  );
}

export interface AgentRuntimeOptions {
  taskId?: string;
  approval: ApprovalHandler;
  onText?: (text: string) => void;
}

export class AgentRuntime {
  private readonly chat = Chat.empty();
  private readonly unrecordedSessionId = randomUUID();
  private readonly requests = new Map<
    number,
    { name: string; arguments: Record<string, unknown> }
  >();
  private toolCallCount = 0;
  private invalidToolCallCount = 0;
  private readonly toolCallRecords = new Map<number, AgentToolCallRecord>();

  private constructor(
    private readonly config: AgentConfig,
    private readonly tools: Tool[],
    private readonly model: Awaited<
      ReturnType<LMStudioClient["llm"]["model"]>
    >,
    private readonly trace: TraceStore | undefined,
    private readonly options: AgentRuntimeOptions,
  ) {
    this.chat.append("system", SYSTEM_PROMPT);
  }

  static async create(
    workspace: string,
    config: AgentConfig,
    options: AgentRuntimeOptions,
  ): Promise<AgentRuntime> {
    const boundary = await WorkspaceBoundary.create(workspace);
    const client = new LMStudioClient();
    let model: Awaited<ReturnType<LMStudioClient["llm"]["model"]>>;
    try {
      model = await client.llm.model(config.model);
    } catch (error) {
      throw new Error(
        "Could not load LM Studio model '" +
          config.model +
          "'. Start LM Studio, download the model, and load it before retrying.",
        { cause: error },
      );
    }

    const tools = await createAgentTools(boundary, config);
    const trace = config.recordSessions
      ? await TraceStore.create(config.dataDirectory, config.maxToolOutputBytes, {
          taskId: options.taskId,
          model: config.model,
          workspace: boundary.root,
          startedAt: new Date().toISOString(),
        })
      : undefined;

    return new AgentRuntime(config, tools, model, trace, options);
  }

  get sessionId(): string {
    return this.trace?.sessionId ?? this.unrecordedSessionId;
  }

  async runOnce(prompt: string): Promise<AgentRunResult> {
    const started = performance.now();
    const startingToolCallCount = this.toolCallCount;
    const startingInvalidToolCallCount = this.invalidToolCallCount;
    const startingCallIds = new Set(this.toolCallRecords.keys());
    let response = "";
    this.chat.append("user", prompt);
    await this.trace?.event({
      kind: "user",
      at: new Date().toISOString(),
      content: prompt,
    });

    try {
      const result = await this.model.act(this.chat, this.tools, {
        maxPredictionRounds: this.config.maxPredictionRounds,
        allowParallelToolExecution: false,
        onPredictionFragment: (fragment: { content?: string }) => {
          if (fragment.content) this.options.onText?.(fragment.content);
        },
        onMessage: (message: { toString(): string }) => {
          this.chat.append(message as never);
          response = message.toString();
        },
        guardToolCall: async (
          _roundIndex: number,
          callId: number,
          controller: {
            toolCallRequest: {
              name: string;
              arguments?: Record<string, unknown>;
            };
            allow(): void;
            deny(reason: string): void;
          },
        ) => {
          const request = {
            name: controller.toolCallRequest.name,
            arguments: controller.toolCallRequest.arguments ?? {},
          };
          this.requests.set(callId, request);
          this.toolCallCount += 1;
          await this.trace?.event({
            kind: "tool_request",
            at: new Date().toISOString(),
            callId,
            ...request,
          });

          const allowed = await authorizeToolCall(
            request.name,
            request.arguments,
            this.options.approval,
          );
          this.toolCallRecords.set(callId, {
            callId,
            ...request,
            allowed,
          });
          await this.trace?.event({
            kind: "tool_decision",
            at: new Date().toISOString(),
            callId,
            allowed,
            reason: allowed ? undefined : "Denied by user.",
          });
          if (allowed) controller.allow();
          else controller.deny("Tool call denied by user.");
        },
        onToolCallResult: async (
          _roundIndex: number,
          callId: number,
          resultValue: unknown,
        ) => {
          const request = this.requests.get(callId);
          const content =
            typeof resultValue === "string"
              ? resultValue
              : JSON.stringify(resultValue);
          const record = this.toolCallRecords.get(callId);
          if (record) record.result = content;
          await this.trace?.event({
            kind: "tool_result",
            at: new Date().toISOString(),
            callId,
            name: request?.name,
            content,
          });
        },
        handleInvalidToolRequest: (
          error: Error,
          request: unknown | undefined,
        ) => {
          this.invalidToolCallCount += 1;
          if (request !== undefined) return error.message;
          throw error;
        },
      });

      response =
        (result as { content?: string }).content?.trim() || response.trim();
      const durationMs = Math.round(performance.now() - started);
      await this.trace?.event({
        kind: "assistant",
        at: new Date().toISOString(),
        content: response,
      });
      await this.trace?.event({
        kind: "completed",
        at: new Date().toISOString(),
        durationMs,
      });
      return {
        response,
        durationMs,
        sessionId: this.sessionId,
        toolCallCount: this.toolCallCount - startingToolCallCount,
        invalidToolCallCount:
          this.invalidToolCallCount - startingInvalidToolCallCount,
        toolCalls: [...this.toolCallRecords.values()].filter(
          call => !startingCallIds.has(call.callId),
        ),
      };
    } catch (error) {
      await this.trace?.event({
        kind: "failed",
        at: new Date().toISOString(),
        message: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  }
}
