import { realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

export class WorkspaceBoundary {
  private constructor(public readonly root: string) {}

  static async create(workspace: string): Promise<WorkspaceBoundary> {
    const root = await realpath(resolve(workspace));
    const info = await stat(root);
    if (!info.isDirectory()) {
      throw new Error("Workspace is not a directory: " + root);
    }
    return new WorkspaceBoundary(root);
  }

  private assertInside(candidate: string): string {
    const rel = relative(this.root, candidate);
    if (rel === "" || (!rel.startsWith(".." + sep) && rel !== ".." && !isAbsolute(rel))) {
      return candidate;
    }
    throw new Error("Path escapes the selected workspace: " + candidate);
  }

  async existing(input: string): Promise<string> {
    const lexical = this.assertInside(resolve(this.root, input));
    return this.assertInside(await realpath(lexical));
  }

  async prospective(input: string): Promise<string> {
    const candidate = this.assertInside(resolve(this.root, input));
    try {
      return await this.existing(input);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
    let parent = dirname(candidate);

    for (;;) {
      try {
        this.assertInside(await realpath(parent));
        return candidate;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
          throw error;
        }
        const next = dirname(parent);
        if (next === parent) {
          throw error;
        }
        parent = next;
      }
    }
  }

  relativePath(absolutePath: string): string {
    return relative(this.root, this.assertInside(resolve(absolutePath)));
  }
}

export function validateRelativePath(input: string): void {
  if (input.length === 0 || isAbsolute(input) || input.includes("\0")) {
    throw new Error("Expected a non-empty relative path.");
  }
  const normalized = input.replaceAll("\\", "/");
  if (
    normalized === ".." ||
    normalized.startsWith("../") ||
    normalized.includes("/../")
  ) {
    throw new Error("Parent traversal is not allowed: " + input);
  }
}
