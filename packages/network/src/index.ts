import net from "node:net";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export async function tcpConnectTest(host: string, port: number, timeoutMs = 2000): Promise<{
  host: string;
  port: number;
  reachable: boolean;
  latencyMs: number;
  error?: string;
}> {
  const started = Date.now();
  return new Promise((resolve) => {
    const socket = new net.Socket();
    let settled = false;

    const done = (reachable: boolean, error?: string) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve({
        host,
        port,
        reachable,
        latencyMs: Date.now() - started,
        error
      });
    };

    socket.setTimeout(timeoutMs);
    socket.once("connect", () => done(true));
    socket.once("error", (e: Error) => done(false, e.message));
    socket.once("timeout", () => done(false, "Connection timeout"));
    socket.connect(port, host);
  });
}

export async function detectRuntimeVersions(): Promise<{
  node: string;
  java: string;
}> {
  let javaVersion = "NOT_FOUND";
  try {
    const { stderr, stdout } = await execFileAsync("java", ["-version"]);
    javaVersion = (stderr || stdout).split(/\r?\n/)[0] ?? "UNKNOWN";
  } catch {
    javaVersion = "NOT_FOUND";
  }
  return {
    node: process.version,
    java: javaVersion
  };
}

export async function listPortUsage(): Promise<
  Array<{ protocol: string; localAddress: string; state: string; pid: number; processName: string }>
> {
  const { stdout } = await execFileAsync("netstat", ["-ano"]);
  const lines = stdout.split(/\r?\n/).slice(4).filter(Boolean);
  const rows = lines
    .map((line: string) => line.trim().split(/\s+/))
    .filter((parts) => parts.length >= 5)
    .map((parts: string[]) => ({
      protocol: parts[0],
      localAddress: parts[1],
      state: parts.length === 5 ? parts[3] : parts[3],
      pid: Number(parts[parts.length - 1])
    }))
    .filter((row: { protocol: string; localAddress: string; state: string; pid: number }) => Number.isFinite(row.pid));

  const uniquePids = [...new Set<number>(rows.map((r) => r.pid))];
  const pidMap = new Map<number, string>();
  await Promise.all(
    uniquePids.map(async (pid: number) => {
      try {
        const { stdout: taskOut } = await execFileAsync("tasklist", ["/FI", `PID eq ${pid}`, "/FO", "CSV", "/NH"]);
        const name = taskOut.split(",")[0]?.replace(/"/g, "") ?? "UNKNOWN";
        pidMap.set(pid, name);
      } catch {
        pidMap.set(pid, "UNKNOWN");
      }
    })
  );

  return rows.map((r: { protocol: string; localAddress: string; state: string; pid: number }) => ({
    protocol: r.protocol,
    localAddress: r.localAddress,
    state: r.state,
    pid: r.pid,
    processName: pidMap.get(r.pid) ?? "UNKNOWN"
  }));
}
