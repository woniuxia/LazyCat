/** 后端 usage 接口原始响应类型 */
export interface PortUsageResponse {
  summary: { total: number; tcp: number; udp: number };
  stateCounts: Record<string, number>;
  processSummaries: Array<{
    pid: number;
    processName: string;
    listeningPorts: string[];
    connectionCount: number;
  }>;
  connections: Array<{
    protocol: string;
    pid: number;
    processName: string;
    localAddress: string;
    remoteAddress: string;
    state: string | null;
  }>;
}

/** 进程详情响应类型 */
export interface PortProcessDetailResponse {
  pid: number;
  name: string;
  path: string;
  commandLine: string;
  startTime: string;
}

export interface PortUsageSummary {
  total: number;
  tcp: number;
  udp: number;
}

export interface PortUsageStateRow {
  state: string;
  count: number;
}

export interface PortUsageProcessRow {
  pid: number;
  processName: string;
  listeningPorts: string[];
  /** 前端派生字段：listeningPorts.join(", ") 或 "-" */
  listeningPortsText: string;
  connectionCount: number;
}

export interface PortUsageConnectionRow {
  protocol: string;
  pid: number;
  processName: string;
  localAddress: string;
  remoteAddress: string;
  state: string;
}
