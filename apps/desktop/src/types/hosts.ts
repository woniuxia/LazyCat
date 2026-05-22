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

/**
 * Save mode passed to `tool:hosts:save`.
 * - `create`: strict insert; duplicate name returns `DUPLICATE_NAME` error.
 * - `update`: update existing row by name; missing name returns error.
 * - `upsert`: legacy behaviour — insert or overwrite silently. Avoid for new
 *   call sites; only kept so older code paths keep working.
 */
export type HostsSaveMode = "create" | "update" | "upsert";

export interface HostsDeleteResult {
  ok: boolean;
  /** True if the deleted profile was the currently activated one. The system
   *  hosts file is **not** reverted automatically; the UI should warn the user. */
  wasActive: boolean;
  /** False if no row matched the name. */
  deleted: boolean;
}
