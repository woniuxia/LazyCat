/**
 * SQL 文本纯函数：语句拆分、只读/危险分类、光标语句提取。
 *
 * 与 Rust 端 `db_drivers/sql_text.rs` 维护同一套规则与测试向量：
 * 前端结论仅用于交互提示，后端始终独立分类并强制拦截。
 */

export type SqlDialect = "mysql" | "pg";

export interface StatementInfo {
  readonly: boolean;
  dml: boolean;
  ddl: boolean;
  verb: string;
  missingWhere: boolean;
}

interface StatementRange {
  text: string;
  start: number;
  end: number;
}

const READONLY_VERBS = new Set(["SELECT", "SHOW", "EXPLAIN", "DESC", "DESCRIBE", "VALUES", "TABLE"]);
const DML_VERBS = new Set(["INSERT", "UPDATE", "DELETE", "REPLACE", "MERGE", "TRUNCATE"]);
const DDL_VERBS = new Set(["CREATE", "ALTER", "DROP", "RENAME", "COMMENT", "GRANT", "REVOKE"]);

function isWordChar(ch: string): boolean {
  return /[A-Za-z0-9_]/.test(ch);
}

/** 若 sql[i] 起始处是字符串/注释/美元引用，返回该段结束位置（不含）；否则返回 -1。 */
function skipNonCode(sql: string, i: number, dialect: SqlDialect): number {
  const ch = sql[i];
  if (ch === "'" || ch === '"' || ch === "`") {
    return skipQuoted(sql, i, ch);
  }
  if (ch === "-" && sql.startsWith("--", i)) {
    const nl = sql.indexOf("\n", i);
    return nl === -1 ? sql.length : nl + 1;
  }
  if (ch === "#" && dialect === "mysql") {
    const nl = sql.indexOf("\n", i);
    return nl === -1 ? sql.length : nl + 1;
  }
  if (ch === "/" && sql.startsWith("/*", i)) {
    const end = sql.indexOf("*/", i + 2);
    return end === -1 ? sql.length : end + 2;
  }
  if (ch === "$" && dialect === "pg") {
    return skipDollarQuoted(sql, i);
  }
  return -1;
}

function skipQuoted(sql: string, start: number, quote: string): number {
  let i = start + 1;
  while (i < sql.length) {
    const ch = sql[i];
    if (ch === "\\" && quote !== "`") {
      i += 2;
      continue;
    }
    if (ch === quote) {
      if (sql[i + 1] === quote) {
        i += 2;
        continue;
      }
      return i + 1;
    }
    i += 1;
  }
  return sql.length;
}

function skipDollarQuoted(sql: string, start: number): number {
  const tagEnd = sql.indexOf("$", start + 1);
  if (tagEnd === -1) return -1;
  const tag = sql.slice(start + 1, tagEnd);
  if (!/^[A-Za-z0-9_]*$/.test(tag)) return -1;
  const closer = `$${tag}$`;
  const end = sql.indexOf(closer, tagEnd + 1);
  return end === -1 ? sql.length : end + closer.length;
}

/** 拆分语句并保留原文偏移（供光标定位复用）。 */
function splitWithRanges(sql: string, dialect: SqlDialect): StatementRange[] {
  const out: StatementRange[] = [];
  let start = 0;
  let i = 0;
  const push = (from: number, to: number) => {
    const raw = sql.slice(from, to);
    const text = raw.trim();
    if (text && !isBlankStatement(text, dialect)) {
      const lead = raw.indexOf(text);
      out.push({ text, start: from + lead, end: from + lead + text.length });
    }
  };
  while (i < sql.length) {
    const skipped = skipNonCode(sql, i, dialect);
    if (skipped !== -1 && skipped > i) {
      i = skipped;
      continue;
    }
    if (sql[i] === ";") {
      push(start, i);
      i += 1;
      start = i;
      continue;
    }
    i += 1;
  }
  push(start, sql.length);
  return out;
}

/** 按分号拆分多语句，跳过字符串/注释/美元引用，返回非空语句文本。 */
export function splitStatements(sql: string, dialect: SqlDialect): string[] {
  return splitWithRanges(sql, dialect).map((r) => r.text);
}

/** 提取光标所在语句（用于"执行光标处语句"）；光标落在语句间隙时取前一条。 */
export function statementAtCursor(sql: string, offset: number, dialect: SqlDialect): string | null {
  const ranges = splitWithRanges(sql, dialect);
  if (ranges.length === 0) return null;
  for (const r of ranges) {
    if (offset >= r.start && offset <= r.end) return r.text;
  }
  const before = ranges.filter((r) => r.end <= offset);
  if (before.length > 0) return before[before.length - 1].text;
  return ranges[0].text;
}

function isBlankStatement(stmt: string, dialect: SqlDialect): boolean {
  return wordsWithDepth(stmt, dialect).length === 0;
}

function wordsWithDepth(stmt: string, dialect: SqlDialect): Array<{ word: string; depth: number }> {
  const out: Array<{ word: string; depth: number }> = [];
  let i = 0;
  let depth = 0;
  let word = "";
  let wordDepth = 0;
  const flush = () => {
    if (word) {
      out.push({ word: word.toUpperCase(), depth: wordDepth });
      word = "";
    }
  };
  while (i < stmt.length) {
    const skipped = skipNonCode(stmt, i, dialect);
    if (skipped !== -1 && skipped > i) {
      flush();
      i = skipped;
      continue;
    }
    const ch = stmt[i];
    if (ch === "(") {
      flush();
      depth += 1;
    } else if (ch === ")") {
      flush();
      depth -= 1;
    } else if (isWordChar(ch)) {
      if (!word) wordDepth = depth;
      word += ch;
    } else {
      flush();
    }
    i += 1;
  }
  flush();
  return out;
}

/** 分类单条语句；规则与 Rust 端一致（含写 CTE 判定）。 */
export function classifyStatement(stmt: string, dialect: SqlDialect): StatementInfo {
  const all = wordsWithDepth(stmt, dialect);
  const top = all.filter((w) => w.depth <= 0).map((w) => w.word);
  const words = top.length > 0 ? top : all.map((w) => w.word);
  const verb = words[0] ?? "";

  let dml = DML_VERBS.has(verb);
  let ddl = DDL_VERBS.has(verb);
  let readonly = READONLY_VERBS.has(verb);

  if (verb === "WITH") {
    dml = words.some((w) => DML_VERBS.has(w));
    ddl = words.some((w) => DDL_VERBS.has(w));
    readonly = !dml && !ddl;
  }

  const missingWhere = (verb === "UPDATE" || verb === "DELETE") && !words.includes("WHERE");

  return { readonly, dml, ddl, verb, missingWhere };
}
