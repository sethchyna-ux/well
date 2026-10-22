import { readFile, writeFile } from "node:fs/promises";
import { tool, type Tool } from "@lmstudio/sdk";
import { z } from "zod";
import { WorkspaceBoundary, validateRelativePath } from "./paths.js";
import { runProcess, type ProcessResult } from "./process.js";
import { sanitize } from "./redact.js";
import type { AgentConfig } from "./types.js";

const ALLOWED_PROGRAMS = new Set([
  "cargo",
  "git",
  "make",
  "node",
  "npm",
  "python3",
  "rg",
  "uv",
]);

export const READ_ONLY_TOOLS = new Set([
  "list_files",
  "search_text",
  "read_file",
  "git_status",
]);

export const TOOL_DEFINITIONS = [
  {
    type: "function",
    function: {
      name: "list_files",
      description:
        "List workspace files, optionally filtered by a ripgrep glob. A listing alone is never enough to conclude that requested code is absent; search or read plausible files next.",
      parameters: {
        type: "object",
        properties: { glob: { type: "string" } },
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "search_text",
      description: "Search workspace text with ripgrep.",
      parameters: {
        type: "object",
        properties: {
          query: { type: "string" },
          glob: { type: "string" },
        },
        required: ["query"],
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "read_file",
      description: "Read a line range from a UTF-8 workspace file.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string" },
          startLine: { type: "integer", minimum: 1 },
          endLine: { type: "integer", minimum: 1 },
        },
        required: ["path"],
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "git_status",
      description: "Return the concise Git status for the selected workspace.",
      parameters: { type: "object", properties: {}, additionalProperties: false },
    },
  },
  {
    type: "function",
    function: {
      name: "replace_text",
      description:
        "Replace one exact, unique text fragment in an existing UTF-8 workspace file. Prefer this for focused edits.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string" },
          oldText: { type: "string" },
          newText: { type: "string" },
        },
        required: ["path", "oldText", "newText"],
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "apply_patch",
      description: "Apply a unified diff inside the selected workspace.",
      parameters: {
        type: "object",
        properties: { patch: { type: "string" } },
        required: ["patch"],
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "run_command",
      description: "Run an allowlisted program directly without a shell.",
      parameters: {
        type: "object",
        properties: {
          program: { type: "string" },
          args: { type: "array", items: { type: "string" } },
          cwd: { type: "string" },
          timeoutMs: { type: "integer", minimum: 1000, maximum: 300000 },
        },
        required: ["program"],
        additionalProperties: false,
      },
    },
  },
] as const;

function describeProcess(result: ProcessResult, maxBytes: number): string {
  const status = result.timedOut
    ? "timed out"
    : "exit code " + String(result.exitCode);
  const parts = ["Process " + status + "."];
  if (result.stdout) parts.push("stdout:\n" + result.stdout);
  if (result.stderr) parts.push("stderr:\n" + result.stderr);
  return sanitize(parts.join("\n"), maxBytes);
}

function validatePatchPaths(patch: string): string[] {
  const paths: string[] = [];
  for (const line of patch.split("\n")) {
    if (!line.startsWith("+++ ") && !line.startsWith("--- ")) continue;
    const raw = line.slice(4).split("\t", 1)[0]?.trim();
    if (!raw || raw === "/dev/null") continue;
    const path = raw.startsWith("a/") || raw.startsWith("b/")
      ? raw.slice(2)
      : raw;
    validateRelativePath(path);
    paths.push(path);
  }
  if (paths.length === 0) {
    throw new Error("Patch contains no file headers.");
  }
  return paths;
}

export async function createAgentTools(
  boundary: WorkspaceBoundary,
  config: AgentConfig,
): Promise<Tool[]> {
  const processOptions = {
    cwd: boundary.root,
    timeoutMs: config.commandTimeoutMs,
    maxOutputBytes: config.maxToolOutputBytes,
  };

  return [
    tool({
      name: "list_files",
      description:
        "List workspace files, optionally filtered by a ripgrep glob. A listing alone is never enough to conclude that requested code is absent; search or read plausible files next.",
      parameters: { glob: z.string().optional() },
      implementation: async ({ glob }) => {
        const args = ["--files"];
        if (glob) args.push("-g", glob);
        const result = await runProcess("rg", args, processOptions);
        if (result.exitCode !== 0 && result.exitCode !== 1) {
          return describeProcess(result, config.maxToolOutputBytes);
        }
        const files = sanitize(result.stdout, config.maxToolOutputBytes);
        return (
          files +
          "\nA file listing is discovery only. Before concluding requested code is absent, call search_text without a glob or read a plausible file."
        );
      },
    }),
    tool({
      name: "search_text",
      description: "Search workspace text with ripgrep.",
      parameters: {
        query: z.string().min(1),
        glob: z.string().optional(),
      },
      implementation: async ({ query, glob }) => {
        const args = ["--line-number", "--no-heading", "--color", "never"];
        if (glob) args.push("--glob", glob);
        args.push("--", query, ".");
        const result = await runProcess("rg", args, processOptions);
        if (result.exitCode !== 0 && result.exitCode !== 1) {
          return describeProcess(result, config.maxToolOutputBytes);
        }
        if (glob && result.exitCode === 1 && !result.stdout) {
          const fallback = await runProcess(
            "rg",
            ["--line-number", "--no-heading", "--color", "never", "--", query, "."],
            processOptions,
          );
          if (fallback.exitCode !== 0 && fallback.exitCode !== 1) {
            return describeProcess(fallback, config.maxToolOutputBytes);
          }
          if (fallback.stdout) {
            return sanitize(
              `No matches under glob ${JSON.stringify(glob)}. Workspace-wide matches:\n${fallback.stdout}\nSearch results identify locations only. Call read_file around the match before answering so nearby declarations and the complete expression are included.`,
              config.maxToolOutputBytes,
            );
          }
        }
        if (result.stdout) {
          return sanitize(
            result.stdout +
              "\nSearch results identify locations only. Call read_file around the match before answering so nearby declarations and the complete expression are included.",
            config.maxToolOutputBytes,
          );
        }
        const candidates = await runProcess("rg", ["--files"], processOptions);
        const candidateText =
          candidates.exitCode === 0 ? candidates.stdout : "";
        return sanitize(
          "No text matches were found. Do not conclude the requested code is absent or repeat phrase searches. Read the most plausible source file from this workspace list:\n" +
            candidateText +
            "\nFor application behavior, terminal behavior, or top-level functions, read src/main.rs first when it exists.",
          config.maxToolOutputBytes,
        );
      },
    }),
    tool({
      name: "read_file",
      description: "Read a line range from a UTF-8 workspace file.",
      parameters: {
        path: z.string().min(1),
        startLine: z.number().int().min(1).optional(),
        endLine: z.number().int().min(1).optional(),
      },
      implementation: async ({ path, startLine = 1, endLine }) => {
        validateRelativePath(path);
        const absolute = await boundary.existing(path);
        const lines = (await readFile(absolute, "utf8")).split("\n");
        const last = endLine ?? Math.min(lines.length, startLine + 399);
        if (last < startLine) throw new Error("endLine precedes startLine.");
        const content = lines
          .slice(startLine - 1, last)
          .map((line, index) => String(startLine + index) + ": " + line)
          .join("\n");
        return sanitize(content, config.maxToolOutputBytes);
      },
    }),
    tool({
      name: "git_status",
      description: "Return the concise Git status for the selected workspace.",
      parameters: {},
      implementation: async () =>
        describeProcess(
          await runProcess(
            "git",
            ["status", "--short", "--branch"],
            processOptions,
          ),
          config.maxToolOutputBytes,
        ),
    }),
    tool({
      name: "replace_text",
      description:
        "Replace one exact, unique text fragment in an existing UTF-8 workspace file. Prefer this for focused edits.",
      parameters: {
        path: z.string().min(1),
        oldText: z.string().min(1).max(512 * 1024),
        newText: z.string().max(512 * 1024),
      },
      implementation: async ({ path, oldText, newText }) => {
        validateRelativePath(path);
        let absolute: string;
        try {
          absolute = await boundary.existing(path);
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "ENOENT") {
            return (
              "Replacement not applied: file does not exist: " +
              path +
              ". Use apply_patch to create a new file."
            );
          }
          throw error;
        }
        const content = await readFile(absolute, "utf8");
        const occurrences = content.split(oldText).length - 1;
        if (occurrences !== 1) {
          return (
            "Replacement not applied: expected exactly one occurrence of oldText, found " +
            String(occurrences) +
            "."
          );
        }
        await writeFile(absolute, content.replace(oldText, newText), "utf8");
        return "Replaced one occurrence in " + path + ".";
      },
    }),
    tool({
      name: "apply_patch",
      description: "Apply a unified diff inside the selected workspace.",
      parameters: { patch: z.string().min(1).max(512 * 1024) },
      implementation: async ({ patch }) => {
        const paths = validatePatchPaths(patch);
        for (const path of paths) await boundary.prospective(path);
        const check = await runProcess(
          "git",
          ["apply", "--recount", "--check", "-"],
          {
            ...processOptions,
            input: patch,
          },
        );
        if (check.exitCode !== 0) {
          return describeProcess(check, config.maxToolOutputBytes);
        }
        const applied = await runProcess("git", ["apply", "--recount", "-"], {
          ...processOptions,
          input: patch,
        });
        return describeProcess(applied, config.maxToolOutputBytes);
      },
    }),
    tool({
      name: "run_command",
      description: "Run an allowlisted program directly without a shell.",
      parameters: {
        program: z.string().min(1),
        args: z.array(z.string().max(16_384)).max(128).optional(),
        cwd: z.string().optional(),
        timeoutMs: z.number().int().min(1_000).max(300_000).optional(),
      },
      implementation: async ({
        program,
        args = [],
        cwd = ".",
        timeoutMs = config.commandTimeoutMs,
      }) => {
        if (!ALLOWED_PROGRAMS.has(program)) {
          throw new Error(
            "Program is not allowlisted. Allowed programs: " +
              [...ALLOWED_PROGRAMS].join(", "),
          );
        }
        const absoluteCwd = await boundary.existing(cwd);
        const result = await runProcess(program, args, {
          cwd: absoluteCwd,
          timeoutMs,
          maxOutputBytes: config.maxToolOutputBytes,
        });
        return describeProcess(result, config.maxToolOutputBytes);
      },
    }),
  ];
}
