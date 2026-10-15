import { spawn } from "node:child_process";

export interface ProcessResult {
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  stdout: string;
  stderr: string;
  timedOut: boolean;
}

function safeChildEnvironment(): NodeJS.ProcessEnv {
  const environment = { ...process.env };
  const secretName =
    /(TOKEN|SECRET|PASSWORD|PASSWD|API_KEY|APIKEY|ACCESS_KEY|PRIVATE_KEY|CREDENTIAL)/i;
  for (const name of Object.keys(environment)) {
    if (secretName.test(name)) delete environment[name];
  }
  return environment;
}

export async function runProcess(
  program: string,
  args: string[],
  options: {
    cwd: string;
    timeoutMs: number;
    maxOutputBytes: number;
    input?: string;
  },
): Promise<ProcessResult> {
  return await new Promise((resolve, reject) => {
    const child = spawn(program, args, {
      cwd: options.cwd,
      env: safeChildEnvironment(),
      shell: false,
      stdio: [options.input === undefined ? "ignore" : "pipe", "pipe", "pipe"],
    });
    const childStdout = child.stdout!;
    const childStderr = child.stderr!;

    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let timedOut = false;

    const collect = (
      chunks: Buffer[],
      current: () => number,
      setCurrent: (value: number) => void,
      chunk: Buffer,
    ) => {
      const remaining = options.maxOutputBytes - current();
      if (remaining <= 0) return;
      const kept = chunk.subarray(0, remaining);
      chunks.push(kept);
      setCurrent(current() + kept.byteLength);
    };

    childStdout.on("data", (chunk: Buffer) =>
      collect(stdout, () => stdoutBytes, value => (stdoutBytes = value), chunk),
    );
    childStderr.on("data", (chunk: Buffer) =>
      collect(stderr, () => stderrBytes, value => (stderrBytes = value), chunk),
    );
    child.on("error", reject);
    if (options.input !== undefined) {
      child.stdin!.end(options.input);
    }

    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
      setTimeout(() => child.kill("SIGKILL"), 1_000).unref();
    }, options.timeoutMs);

    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      resolve({
        exitCode,
        signal,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
        timedOut,
      });
    });
  });
}
