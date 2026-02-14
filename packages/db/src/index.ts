import path from "node:path";
import os from "node:os";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import initSqlJs, { type Database, type SqlJsStatic } from "sql.js";

let SQL: SqlJsStatic | null = null;
let db: Database | null = null;
let dbPath = "";
const require = createRequire(import.meta.url);

function getDatabasePath(): string {
  const dir = path.join(os.homedir(), ".lazycat");
  mkdirSync(dir, { recursive: true });
  return path.join(dir, "lazycat.sqlite");
}

function getWasmPath(): string {
  try {
    return require.resolve("sql.js/dist/sql-wasm.wasm");
  } catch {
    return path.join(process.cwd(), "node_modules", "sql.js", "dist", "sql-wasm.wasm");
  }
}

async function getSql(): Promise<SqlJsStatic> {
  if (SQL) return SQL;
  SQL = await initSqlJs({
    locateFile: () => getWasmPath()
  });
  return SQL;
}

export async function initDatabase(): Promise<void> {
  if (db) return;
  const sql = await getSql();
  dbPath = getDatabasePath();

  if (existsSync(dbPath)) {
    const data = readFileSync(dbPath);
    db = new sql.Database(data);
  } else {
    db = new sql.Database();
  }

  db.run(`
    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS tool_history (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      tool_id TEXT NOT NULL,
      input_digest TEXT NOT NULL,
      summary TEXT,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS hosts_profiles (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL UNIQUE,
      content TEXT NOT NULL,
      enabled INTEGER NOT NULL DEFAULT 0,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS hosts_backups (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      profile_name TEXT NOT NULL,
      backup_path TEXT NOT NULL,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS regex_templates (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      expression TEXT NOT NULL,
      category TEXT NOT NULL
    );
  `);

  flushDatabase();
}

export function getDb(): Database {
  if (!db) {
    throw new Error("Database not initialized. Call initDatabase() first.");
  }
  return db;
}

export function flushDatabase(): void {
  if (!db) return;
  const data = db.export();
  writeFileSync(dbPath, Buffer.from(data));
}

export interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean;
  updatedAt: string;
}

export function saveHostsProfile(name: string, content: string): void {
  const database = getDb();
  const escapedName = name.replace(/'/g, "''");
  const escapedContent = content.replace(/'/g, "''");
  database.run(`
    INSERT INTO hosts_profiles (name, content, enabled, updated_at)
    VALUES ('${escapedName}', '${escapedContent}', 0, CURRENT_TIMESTAMP)
    ON CONFLICT(name) DO UPDATE SET
      content = excluded.content,
      updated_at = CURRENT_TIMESTAMP
  `);
  flushDatabase();
}

export function listHostsProfiles(): HostsProfile[] {
  const database = getDb();
  const result = database.exec(`
    SELECT id, name, content, enabled, updated_at
    FROM hosts_profiles
    ORDER BY updated_at DESC
  `);

  if (result.length === 0) return [];
  const rows = result[0];
  const idx = (column: string) => rows.columns.indexOf(column);
  return rows.values.map((valueRow) => ({
    id: Number(valueRow[idx("id")]),
    name: String(valueRow[idx("name")]),
    content: String(valueRow[idx("content")]),
    enabled: Number(valueRow[idx("enabled")]) === 1,
    updatedAt: String(valueRow[idx("updated_at")])
  }));
}

export function getHostsProfileByName(name: string): HostsProfile | null {
  const escapedName = name.replace(/'/g, "''");
  const database = getDb();
  const result = database.exec(`
    SELECT id, name, content, enabled, updated_at
    FROM hosts_profiles
    WHERE name = '${escapedName}'
    LIMIT 1
  `);
  if (result.length === 0 || result[0].values.length === 0) return null;
  const row = result[0];
  const valueRow = row.values[0];
  const idx = (column: string) => row.columns.indexOf(column);
  return {
    id: Number(valueRow[idx("id")]),
    name: String(valueRow[idx("name")]),
    content: String(valueRow[idx("content")]),
    enabled: Number(valueRow[idx("enabled")]) === 1,
    updatedAt: String(valueRow[idx("updated_at")])
  };
}

export function removeHostsProfile(name: string): void {
  const escapedName = name.replace(/'/g, "''");
  const database = getDb();
  database.run(`DELETE FROM hosts_profiles WHERE name = '${escapedName}'`);
  flushDatabase();
}

export function markHostsProfileEnabled(name: string): void {
  const escapedName = name.replace(/'/g, "''");
  const database = getDb();
  database.run(`UPDATE hosts_profiles SET enabled = 0`);
  database.run(`UPDATE hosts_profiles SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE name = '${escapedName}'`);
  flushDatabase();
}
