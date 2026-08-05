// @vitest-environment happy-dom
import { createApp, nextTick, type App } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ElTable, ElTableColumn } from "element-plus";
import HttpStatusTable from "./HttpStatusTable.vue";
import type { HttpStatusCode } from "../types/httpStatus";

const mountedApps: App[] = [];

const rows: HttpStatusCode[] = [
  {
    code: 200,
    name: "OK",
    desc: "成功",
    usage: "请求成功",
    causes: "",
    explanation: "请求已成功处理。",
    troubleshooting: "检查响应。",
    responseHeaders: [],
  },
  {
    code: 404,
    name: "Not Found",
    desc: "未找到",
    usage: "资源不存在",
    causes: "路径错误",
    explanation: "服务器找不到资源。",
    troubleshooting: "核对 URL。",
    responseHeaders: [],
  },
];

afterEach(() => {
  for (const app of mountedApps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

describe("HttpStatusTable", () => {
  it("forwards a real table row click as an expansion change", async () => {
    const onExpandChange = vi.fn();
    const root = document.createElement("div");
    document.body.appendChild(root);
    const app = createApp(HttpStatusTable, {
      data: rows,
      expandedCodes: [],
      showHeader: true,
      onExpandChange,
    });
    app.component("ElTable", ElTable);
    app.component("ElTableColumn", ElTableColumn);
    app.mount(root);
    mountedApps.push(app);
    await nextTick();
    await nextTick();

    const tableRows = root.querySelectorAll(".el-table__body-wrapper tbody .el-table__row");
    expect(tableRows).toHaveLength(2);
    tableRows[1].dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await nextTick();

    expect(onExpandChange).toHaveBeenCalledTimes(1);
    expect(onExpandChange.mock.calls[0][0]).toMatchObject({ code: 404 });
    expect(onExpandChange.mock.calls[0][1].map((row: HttpStatusCode) => row.code)).toEqual([
      200, 404,
    ]);
  });
});
