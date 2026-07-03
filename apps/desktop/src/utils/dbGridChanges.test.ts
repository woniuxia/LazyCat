import { describe, expect, it } from "vitest";
import { buildGridChanges, renderChangeSql, summarizeChanges } from "./dbGridChanges";
import type { DbColumnMeta, DbGridChange } from "../types/db";

const columns: DbColumnMeta[] = [
  { name: "id", typeName: "INT", kind: "number" },
  { name: "name", typeName: "VARCHAR", kind: "text" },
  { name: "memo", typeName: "TEXT", kind: "text" },
];

const rows = [
  ["1", "alice", null],
  ["2", "bob", "x"],
  ["3", "carol", "y"],
];

describe("buildGridChanges", () => {
  it("归一化删除、更新、新增，顺序为删→改→增", () => {
    const changes = buildGridChanges({
      columns,
      pkColumns: ["id"],
      rows,
      edits: [{ rowIndex: 1, values: { name: "bobby", memo: null } }],
      inserts: [{ name: "dave" }],
      deletes: [2],
    });
    expect(changes.map((c) => c.type)).toEqual(["delete", "update", "insert"]);
    expect(changes[0].pk).toEqual({ id: "3" });
    expect(changes[1].pk).toEqual({ id: "2" });
    expect(changes[1].values).toEqual({ name: "bobby", memo: null });
    expect(changes[2].values).toEqual({ name: "dave" });
  });

  it("同一行既编辑又删除时删除优先", () => {
    const changes = buildGridChanges({
      columns,
      pkColumns: ["id"],
      rows,
      edits: [{ rowIndex: 0, values: { name: "zz" } }],
      inserts: [],
      deletes: [0],
    });
    expect(changes).toHaveLength(1);
    expect(changes[0].type).toBe("delete");
  });

  it("主键取原始行值（编辑前）", () => {
    const changes = buildGridChanges({
      columns,
      pkColumns: ["id"],
      rows,
      edits: [{ rowIndex: 0, values: { id: "99" } }],
      inserts: [],
      deletes: [],
    });
    expect(changes[0].pk).toEqual({ id: "1" });
    expect(changes[0].values).toEqual({ id: "99" });
  });

  it("无主键或主键为空时报错", () => {
    expect(() =>
      buildGridChanges({ columns, pkColumns: [], rows, edits: [], inserts: [], deletes: [0] })
    ).toThrow("没有主键");
    const nullPkRows = [[null, "a", "b"]];
    expect(() =>
      buildGridChanges({
        columns,
        pkColumns: ["id"],
        rows: nullPkRows,
        edits: [],
        inserts: [],
        deletes: [0],
      })
    ).toThrow("主键列 id 为空");
  });

  it("空编辑与空新增被跳过", () => {
    const changes = buildGridChanges({
      columns,
      pkColumns: ["id"],
      rows,
      edits: [{ rowIndex: 0, values: {} }],
      inserts: [{}],
      deletes: [],
    });
    expect(changes).toHaveLength(0);
  });
});

describe("renderChangeSql", () => {
  it("MySQL 用反引号并带库名前缀，NULL 渲染为字面量", () => {
    const change: DbGridChange = {
      type: "update",
      pk: { id: "5" },
      values: { name: "o'x", memo: null },
    };
    expect(renderChangeSql(change, "users", "app", "mysql")).toBe(
      "UPDATE `app`.`users` SET `name` = 'o''x', `memo` = NULL WHERE `id` = '5';"
    );
  });

  it("KingbaseES 表名自带 schema 限定，用双引号", () => {
    const insert: DbGridChange = { type: "insert", pk: {}, values: { name: "a" } };
    expect(renderChangeSql(insert, "public.users", "appdb", "kingbase")).toBe(
      'INSERT INTO "public"."users" ("name") VALUES (\'a\');'
    );
    const del: DbGridChange = { type: "delete", pk: { id: "3" }, values: {} };
    expect(renderChangeSql(del, "public.users", "appdb", "kingbase")).toBe(
      'DELETE FROM "public"."users" WHERE "id" = \'3\';'
    );
  });
});

describe("summarizeChanges", () => {
  it("汇总变更数量", () => {
    const changes: DbGridChange[] = [
      { type: "update", pk: { id: "1" }, values: { a: "1" } },
      { type: "delete", pk: { id: "2" }, values: {} },
      { type: "delete", pk: { id: "3" }, values: {} },
    ];
    expect(summarizeChanges(changes)).toBe("更新 1 行，删除 2 行");
    expect(summarizeChanges([])).toBe("无变更");
  });
});
