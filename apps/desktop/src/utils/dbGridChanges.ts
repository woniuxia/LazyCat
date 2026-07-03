/**
 * 表数据网格暂存变更集纯函数：
 * 把"编辑单元格 / 新增行 / 删除行"的暂存状态归一化为后端 `table_apply_changes`
 * 需要的变更列表，并渲染仅供预览展示的 SQL 文本（真实执行由后端参数化构造）。
 */

import type { DbCellValue, DbColumnMeta, DbGridChange } from "../types/db";

export interface StagedEditInput {
  /** 当前页行下标 */
  rowIndex: number;
  /** 仅包含被修改的列 → 新值 */
  values: Record<string, DbCellValue>;
}

export interface BuildChangesInput {
  columns: DbColumnMeta[];
  pkColumns: string[];
  /** 当前页原始行数据（未套用编辑） */
  rows: DbCellValue[][];
  edits: StagedEditInput[];
  inserts: Array<Record<string, DbCellValue>>;
  deletes: number[];
}

/**
 * 归一化暂存状态为变更列表。
 * 顺序固定为：删除 → 更新 → 新增（先释放唯一键，避免行替换场景冲突）。
 * 同一行既被编辑又被删除时，删除优先。
 */
export function buildGridChanges(input: BuildChangesInput): DbGridChange[] {
  const { columns, pkColumns, rows, edits, inserts, deletes } = input;
  if (pkColumns.length === 0) {
    throw new Error("该表没有主键，无法应用网格编辑");
  }
  const colIndex = new Map(columns.map((c, i) => [c.name, i]));
  for (const pk of pkColumns) {
    if (!colIndex.has(pk)) {
      throw new Error(`结果集中缺少主键列 ${pk}，无法定位行`);
    }
  }

  const pkOf = (rowIndex: number): Record<string, DbCellValue> => {
    const row = rows[rowIndex];
    if (!row) {
      throw new Error(`行下标 ${rowIndex} 超出当前页范围`);
    }
    const pk: Record<string, DbCellValue> = {};
    for (const name of pkColumns) {
      const value = row[colIndex.get(name) as number];
      if (value === null || value === undefined) {
        throw new Error(`第 ${rowIndex + 1} 行主键列 ${name} 为空，无法定位行`);
      }
      pk[name] = value;
    }
    return pk;
  };

  const deleted = new Set(deletes);
  const changes: DbGridChange[] = [];

  for (const rowIndex of deletes) {
    changes.push({ type: "delete", pk: pkOf(rowIndex), values: {} });
  }
  for (const edit of edits) {
    if (deleted.has(edit.rowIndex)) continue;
    if (Object.keys(edit.values).length === 0) continue;
    changes.push({ type: "update", pk: pkOf(edit.rowIndex), values: { ...edit.values } });
  }
  for (const insert of inserts) {
    const values = Object.fromEntries(
      Object.entries(insert).filter(([, v]) => v !== undefined)
    ) as Record<string, DbCellValue>;
    if (Object.keys(values).length === 0) continue;
    changes.push({ type: "insert", pk: {}, values });
  }
  return changes;
}

function quoteIdent(name: string, engine: "mysql" | "kingbase"): string {
  if (engine === "mysql") {
    return `\`${name.replace(/`/g, "``")}\``;
  }
  return name
    .split(".")
    .map((part) => `"${part.replace(/"/g, '""')}"`)
    .join(".");
}

function renderValue(value: DbCellValue): string {
  if (value === null) return "NULL";
  return `'${value.replace(/'/g, "''")}'`;
}

/** 渲染单条变更的预览 SQL（仅展示用；后端执行时按列类型参数化构造）。 */
export function renderChangeSql(
  change: DbGridChange,
  table: string,
  database: string,
  engine: "mysql" | "kingbase"
): string {
  // KingbaseES 的表名自带 schema 限定；MySQL 需要库名前缀
  const qualified =
    engine === "mysql"
      ? `${quoteIdent(database, engine)}.${quoteIdent(table, engine)}`
      : quoteIdent(table, engine);
  switch (change.type) {
    case "update": {
      const sets = Object.entries(change.values)
        .map(([col, v]) => `${quoteIdent(col, engine)} = ${renderValue(v)}`)
        .join(", ");
      const wheres = Object.entries(change.pk)
        .map(([col, v]) => `${quoteIdent(col, engine)} = ${renderValue(v)}`)
        .join(" AND ");
      return `UPDATE ${qualified} SET ${sets} WHERE ${wheres};`;
    }
    case "insert": {
      const cols = Object.keys(change.values)
        .map((c) => quoteIdent(c, engine))
        .join(", ");
      const values = Object.values(change.values).map(renderValue).join(", ");
      return `INSERT INTO ${qualified} (${cols}) VALUES (${values});`;
    }
    case "delete": {
      const wheres = Object.entries(change.pk)
        .map(([col, v]) => `${quoteIdent(col, engine)} = ${renderValue(v)}`)
        .join(" AND ");
      return `DELETE FROM ${qualified} WHERE ${wheres};`;
    }
  }
}

/** 汇总变更数量文案，用于确认弹窗标题。 */
export function summarizeChanges(changes: DbGridChange[]): string {
  const count = { update: 0, insert: 0, delete: 0 };
  for (const c of changes) count[c.type] += 1;
  const parts: string[] = [];
  if (count.update) parts.push(`更新 ${count.update} 行`);
  if (count.insert) parts.push(`新增 ${count.insert} 行`);
  if (count.delete) parts.push(`删除 ${count.delete} 行`);
  return parts.join("，") || "无变更";
}
