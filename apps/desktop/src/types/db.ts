/** 数据库工作台类型定义 */

export type DbEngine = "mysql" | "kingbase" | "redis";

export type DbEnvTag = "dev" | "test" | "prod" | "other";

export interface DbConnectionOptions {
  /** 查询超时（秒），默认 30 */
  timeoutSecs?: number;
  /** 查询行数上限，默认 1000 */
  maxRows?: number;
  /** 连接超时（秒），默认 8 */
  connectTimeoutSecs?: number;
}

export interface DbConnection {
  id: string;
  name: string;
  engine: DbEngine;
  host: string;
  port: number;
  username: string;
  hasPassword: boolean;
  defaultDatabase: string | null;
  envTag: DbEnvTag;
  readOnly: boolean;
  groupName: string | null;
  sortOrder: number;
  options: DbConnectionOptions;
  lastUsedAt: number | null;
}

/** 连接编辑表单（password 语义：undefined=保持不变，空串=清除，非空=更新） */
export interface DbConnectionDraft {
  id?: string;
  name: string;
  engine: DbEngine;
  host: string;
  port: number;
  username: string;
  password?: string;
  defaultDatabase?: string;
  envTag: DbEnvTag;
  readOnly: boolean;
  groupName?: string;
  options?: DbConnectionOptions;
}

export type DbColumnKind = "number" | "text" | "datetime" | "bool" | "binary" | "json";

export interface DbColumnMeta {
  name: string;
  typeName: string;
  kind: DbColumnKind;
}

/** 单元格值统一为 string | null（后端已字符串化，保精度） */
export type DbCellValue = string | null;

export interface DbStatementResult {
  sql: string;
  columns: DbColumnMeta[];
  rows: DbCellValue[][];
  affected: number;
  truncated: boolean;
  durationMs: number;
}

export interface DbConfirmReason {
  kind: "prodWrite" | "missingWhere";
  statementIndex: number;
  verb: string;
  preview: string;
}

/** query_execute / table_apply_changes 的两段式确认响应 */
export interface DbNeedsConfirmation {
  needsConfirmation: true;
  reasons: DbConfirmReason[];
}

export interface DbQueryExecuteResponse {
  results: DbStatementResult[];
  error: { statementIndex: number; message: string } | null;
}

export interface DbTableBrief {
  name: string;
  tableType: "table" | "view";
  comment: string;
  rowEstimate: number;
}

export interface DbColumnDetail {
  name: string;
  dataType: string;
  nullable: boolean;
  defaultValue: string | null;
  comment: string;
  primaryKey: boolean;
}

export interface DbIndexDetail {
  name: string;
  columns: string[];
  unique: boolean;
  definition: string;
}

export interface DbTableDetail {
  columns: DbColumnDetail[];
  indexes: DbIndexDetail[];
  ddl: string;
}

export type DbFilterOp =
  | "="
  | "<>"
  | ">"
  | "<"
  | ">="
  | "<="
  | "LIKE"
  | "NOT LIKE"
  | "IS NULL"
  | "IS NOT NULL";

export interface DbDataFilter {
  column: string;
  op: DbFilterOp;
  value: string;
}

export interface DbTableDataResponse {
  result: DbStatementResult;
  total: number;
  page: number;
  pageSize: number;
}

export type DbGridChangeType = "update" | "insert" | "delete";

export interface DbGridChange {
  type: DbGridChangeType;
  pk: Record<string, DbCellValue>;
  values: Record<string, DbCellValue>;
}

export interface DbApplyChangesResponse {
  ok: boolean;
  applied?: number[];
  failedIndex?: number;
  message?: string;
  needsConfirmation?: boolean;
  reasons?: DbConfirmReason[];
}

export interface DbSavedQuery {
  id: string;
  connectionId: string | null;
  title: string;
  sql: string;
  updatedAt: number;
}

export interface DbHistoryEntry {
  id: number;
  connectionId: string;
  sql: string;
  executedAt: number;
  durationMs: number | null;
  status: "ok" | "error";
  rowCount: number | null;
}

// ---------- Redis（二期） ----------

export type RedisKeyType = "string" | "hash" | "list" | "set" | "zset" | "stream" | string;

export interface RedisScanItem {
  key: string;
  type: RedisKeyType;
}

export interface RedisScanResponse {
  cursor: number;
  done: boolean;
  keys: RedisScanItem[];
}

export interface RedisHashEntry {
  field: string;
  value: string;
}

export interface RedisZsetEntry {
  member: string;
  score: number;
}

export interface RedisKeyDetail {
  key: string;
  type: RedisKeyType;
  /** 秒；-1 永不过期，-2 不存在 */
  ttl: number;
  encoding: string;
  memory: number | null;
  value: string | string[] | RedisHashEntry[] | RedisZsetEntry[];
  total: number;
  truncated: boolean;
}

export interface RedisCommandResponse {
  result: unknown;
  durationMs: number;
}

export const DB_ENGINE_LABELS: Record<DbEngine, string> = {
  mysql: "MySQL",
  kingbase: "KingbaseES",
  redis: "Redis",
};

export const DB_ENGINE_DEFAULT_PORTS: Record<DbEngine, number> = {
  mysql: 3306,
  kingbase: 54321,
  redis: 6379,
};

export const DB_ENV_LABELS: Record<DbEnvTag, string> = {
  dev: "开发",
  test: "测试",
  prod: "生产",
  other: "其他",
};

/** 环境标签色点（浅色主题设计色） */
export const DB_ENV_COLORS: Record<DbEnvTag, string> = {
  dev: "#67c23a",
  test: "#e6a23c",
  prod: "#f56c6c",
  other: "#909399",
};
