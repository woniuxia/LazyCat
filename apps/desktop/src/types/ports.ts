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
