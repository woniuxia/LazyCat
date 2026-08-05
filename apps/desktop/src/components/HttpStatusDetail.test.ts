// @vitest-environment happy-dom
import { createApp, nextTick, type App } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import HttpStatusDetail from "./HttpStatusDetail.vue";

const mountedApps: App[] = [];

afterEach(() => {
  for (const app of mountedApps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

describe("HttpStatusDetail", () => {
  it("renders practical detail and hides empty response-header sections", async () => {
    const root = document.createElement("div");
    document.body.appendChild(root);
    const app = createApp(HttpStatusDetail, {
      status: {
        code: 429,
        name: "Too Many Requests",
        desc: "请求过多",
        usage: "触发限流",
        causes: "频率过高; 重试风暴",
        explanation: "服务器正在限制请求频率。",
        troubleshooting: "降低频率; 按 Retry-After 退避",
        responseHeaders: [{ name: "Retry-After", description: "等待后重试。" }],
      },
    });
    app.mount(root);
    mountedApps.push(app);
    await nextTick();

    expect(root.textContent).toContain("服务器正在限制请求频率");
    expect(root.textContent).toContain("Retry-After");

    app.unmount();
    mountedApps.pop();
    const emptyRoot = document.createElement("div");
    document.body.appendChild(emptyRoot);
    const emptyApp = createApp(HttpStatusDetail, {
      status: {
        code: 200,
        name: "OK",
        desc: "成功",
        usage: "请求成功",
        causes: "",
        explanation: "请求已成功处理。",
        troubleshooting: "检查业务响应。",
        responseHeaders: [],
      },
    });
    emptyApp.mount(emptyRoot);
    mountedApps.push(emptyApp);
    await nextTick();

    expect(emptyRoot.textContent).not.toContain("相关响应头");
  });
});
