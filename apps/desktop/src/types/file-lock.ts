export interface FileLockProcess {
  pid: number;
  appName: string;
  appType: string;
  status: string;
  executablePath: string | null;
  startedAt: string | null;
}

export interface FileLockInspectResponse {
  path: string;
  canonicalPath: string;
  scannedAt: string;
  processes: FileLockProcess[];
  warnings: string[];
}
