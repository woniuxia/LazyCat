import { describe, expect, it } from "vitest";
import { classifyStatement, splitStatements, statementAtCursor } from "./dbSqlClassify";

/** 与 Rust 端 sql_text.rs 共享的测试向量，两端行为必须一致 */
describe("splitStatements", () => {
  it("按分号拆分基础语句", () => {
    expect(splitStatements("SELECT 1; SELECT 2", "mysql")).toEqual(["SELECT 1", "SELECT 2"]);
    expect(splitStatements("SELECT 1", "mysql")).toEqual(["SELECT 1"]);
    expect(splitStatements("  ;;  ", "mysql")).toEqual([]);
  });

  it("跳过字符串与注释中的分号", () => {
    expect(splitStatements("SELECT ';' AS a; SELECT 2", "mysql")).toEqual([
      "SELECT ';' AS a",
      "SELECT 2",
    ]);
    expect(splitStatements("-- x; y\nSELECT 1", "mysql")).toEqual(["-- x; y\nSELECT 1"]);
    expect(splitStatements("/* ; */ SELECT 1", "mysql")).toEqual(["/* ; */ SELECT 1"]);
    expect(splitStatements("SELECT `a;b` FROM t", "mysql")).toEqual(["SELECT `a;b` FROM t"]);
    expect(splitStatements('SELECT "a;b" FROM t; SELECT 2', "pg")).toEqual([
      'SELECT "a;b" FROM t',
      "SELECT 2",
    ]);
    expect(splitStatements("SELECT 'it\\'s; fine'; SELECT 2", "mysql")).toEqual([
      "SELECT 'it\\'s; fine'",
      "SELECT 2",
    ]);
    expect(splitStatements("SELECT 'a''b;c'; SELECT 2", "mysql")).toEqual([
      "SELECT 'a''b;c'",
      "SELECT 2",
    ]);
  });

  it("# 注释仅 MySQL 生效，PG 的 #> 是 JSONB 操作符", () => {
    expect(splitStatements("SELECT 1 # c;\n; SELECT 2", "mysql")).toHaveLength(2);
    expect(splitStatements("SELECT data #> '{a}' FROM t; SELECT 2", "pg")).toEqual([
      "SELECT data #> '{a}' FROM t",
      "SELECT 2",
    ]);
  });

  it("PG 美元引用不被分号拆断", () => {
    const sql =
      "CREATE FUNCTION f() RETURNS void AS $$ BEGIN PERFORM 1; END $$ LANGUAGE plpgsql; SELECT 1";
    const parts = splitStatements(sql, "pg");
    expect(parts).toHaveLength(2);
    expect(parts[0]).toContain("PERFORM 1;");
    expect(splitStatements("SELECT $tag$a;b$tag$; SELECT 2", "pg")).toHaveLength(2);
    expect(splitStatements("SELECT $1; SELECT 2", "pg")).toHaveLength(2);
  });

  it("纯注释语句被丢弃", () => {
    expect(splitStatements("-- hello\n; SELECT 1", "mysql")).toEqual(["SELECT 1"]);
  });
});

describe("classifyStatement", () => {
  it("识别只读形态", () => {
    for (const sql of [
      "SELECT * FROM t",
      "  select 1",
      "(SELECT 1)",
      "SHOW TABLES",
      "EXPLAIN SELECT 1",
      "DESC t",
      "DESCRIBE t",
      "VALUES (1)",
      "WITH a AS (SELECT 1) SELECT * FROM a",
      "/* note */ SELECT 1",
    ]) {
      const info = classifyStatement(sql, "mysql");
      expect(info.readonly, sql).toBe(true);
      expect(info.dml || info.ddl, sql).toBe(false);
    }
  });

  it("识别写语句（含写 CTE）", () => {
    expect(classifyStatement("INSERT INTO t VALUES (1)", "mysql").dml).toBe(true);
    const writeCte = classifyStatement("WITH a AS (SELECT 1) INSERT INTO t SELECT * FROM a", "pg");
    expect(writeCte.dml).toBe(true);
    expect(writeCte.readonly).toBe(false);
    expect(classifyStatement("TRUNCATE TABLE t", "mysql").dml).toBe(true);
    expect(classifyStatement("CREATE TABLE t (id INT)", "mysql").ddl).toBe(true);
    expect(classifyStatement("SELECT 'INSERT INTO x' AS s", "mysql").readonly).toBe(true);
  });

  it("检测顶层缺失 WHERE", () => {
    expect(classifyStatement("UPDATE t SET a=1", "mysql").missingWhere).toBe(true);
    expect(classifyStatement("DELETE FROM t", "mysql").missingWhere).toBe(true);
    expect(classifyStatement("UPDATE t SET a=1 WHERE id=1", "mysql").missingWhere).toBe(false);
    expect(
      classifyStatement("DELETE FROM t WHERE id IN (SELECT id FROM x WHERE y=1)", "mysql")
        .missingWhere
    ).toBe(false);
    expect(
      classifyStatement("UPDATE t SET a=(SELECT max(w) FROM x WHERE q=1)", "mysql").missingWhere
    ).toBe(true);
    expect(classifyStatement("UPDATE t SET a='WHERE'", "mysql").missingWhere).toBe(true);
    expect(classifyStatement("INSERT INTO t VALUES (1)", "mysql").missingWhere).toBe(false);
  });
});

describe("statementAtCursor", () => {
  const sql = "SELECT 1;\nUPDATE t SET a=1 WHERE id=1;\nSELECT 3";

  it("光标落在语句内取该语句", () => {
    expect(statementAtCursor(sql, 2, "mysql")).toBe("SELECT 1");
    expect(statementAtCursor(sql, sql.indexOf("UPDATE") + 3, "mysql")).toContain("UPDATE t");
    expect(statementAtCursor(sql, sql.length, "mysql")).toBe("SELECT 3");
  });

  it("光标落在间隙取前一条，空文本返回 null", () => {
    expect(statementAtCursor(sql, sql.indexOf("\nUPDATE"), "mysql")).toBe("SELECT 1");
    expect(statementAtCursor("", 0, "mysql")).toBeNull();
    expect(statementAtCursor("   ", 1, "mysql")).toBeNull();
  });
});
