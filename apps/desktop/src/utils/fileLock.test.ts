import { describe, expect, it } from "vitest";
import type { FileLockProcess } from "../types";
import {
  fileLockAppTypeLabel,
  fileLockPathsMatch,
  fileLockStatusLabel,
  filterAndSortFileLockProcesses,
  normalizeFileLockPath,
} from "./fileLock";

const processes: FileLockProcess[] = [
  {
    pid: 4020,
    appName: "Java",
    appType: "service",
    status: "running",
    executablePath: "C:\\Program Files\\Java\\bin\\java.exe",
  },
  {
    pid: 120,
    appName: "Code.exe",
    appType: "main-window",
    status: "stopped",
    executablePath: null,
  },
  {
    pid: 80,
    appName: "Explorer",
    appType: "explorer",
    status: "running",
    executablePath: "C:\\Windows\\explorer.exe",
  },
];

describe("file lock utilities", () => {
  it("matches Windows paths case-insensitively across slash styles", () => {
    expect(normalizeFileLockPath(" C:/Work/demo\\target\\app.jar\\ ")).toBe(
      "c:\\work\\demo\\target\\app.jar",
    );
    expect(fileLockPathsMatch("C:/Work/demo/app.jar", "c:\\work\\demo\\app.jar\\")).toBe(true);
    expect(fileLockPathsMatch("C:/Work/demo/app.jar", "C:/Work/other.jar")).toBe(false);
  });

  it("searches translated labels and executable paths without mutating input", () => {
    const result = filterAndSortFileLockProcesses(processes, "服务", "pid-asc");

    expect(result.map((item) => item.pid)).toEqual([4020]);
    expect(processes.map((item) => item.pid)).toEqual([4020, 120, 80]);
    expect(filterAndSortFileLockProcesses(processes, "explorer.exe", "pid-asc")).toHaveLength(1);
  });

  it("sorts by PID, application, and status deterministically", () => {
    expect(
      filterAndSortFileLockProcesses(processes, "", "pid-asc").map((item) => item.pid),
    ).toEqual([80, 120, 4020]);
    expect(
      filterAndSortFileLockProcesses(processes, "", "pid-desc").map((item) => item.pid),
    ).toEqual([4020, 120, 80]);
    expect(
      filterAndSortFileLockProcesses(processes, "", "app").map((item) => item.appName),
    ).toEqual(["Code.exe", "Explorer", "Java"]);
  });

  it("uses explicit labels and preserves unknown values", () => {
    expect(fileLockAppTypeLabel("service")).toBe("服务");
    expect(fileLockStatusLabel("running")).toBe("运行中");
    expect(fileLockStatusLabel("custom-status")).toBe("custom-status");
  });
});
