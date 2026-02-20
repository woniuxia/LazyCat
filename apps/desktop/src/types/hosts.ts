export interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean;
  updatedAt: string;
}

export interface HostsBackupEntry {
  filename: string;
  size: number;
  modifiedAt: string;
}
