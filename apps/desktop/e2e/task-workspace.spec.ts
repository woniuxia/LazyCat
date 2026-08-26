import { expect, test, type Locator, type Page } from "@playwright/test";

interface WorkspaceGeometry {
  switchBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
  sidebarBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
  listBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
  detailBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
  toolbarBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
  listContentBox: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;
}

async function installTaskBridge(page: Page) {
  await page.addInitScript(() => {
    const callbacks = new Map<number, (data: unknown) => void>();
    let callbackId = 1;
    const todoItem = {
      id: 1,
      rootId: 1,
      kind: "one_off",
      pinned: false,
      title: "完成任务工作区视觉验收",
      typeId: null,
      priority: "P1",
      description: "核对两个视图的布局锚点",
      status: "pending",
      eventAt: "2026-08-20T10:00:00+08:00",
      reminderPresets: ["none"],
      snoozeUntil: null,
      lastNotifiedAt: null,
      displayAt: null,
      assignees: [],
      links: [],
      isOverdue: false,
      recurrence: null,
      completedAt: null,
      createdAt: "2026-08-19T00:00:00Z",
      updatedAt: "2026-08-19T00:00:00Z",
    };
    let todoItems = [todoItem];
    const followUpItem = {
      id: 1,
      title: "确认外部接口交付",
      description: "等待责任人反馈",
      expectedOutcome: "接口通过验收",
      priority: "P1",
      attentionStatus: "active",
      externalResult: "unknown",
      endingMode: null,
      personId: 1,
      personName: "张三",
      personNameSnapshot: "张三",
      reviewAt: "2026-08-18T09:00:00+08:00",
      expectedCompletionAt: null,
      snoozeUntil: null,
      lastNotifiedReviewAt: null,
      endedAt: null,
      createdAt: "2026-08-19T00:00:00Z",
      updatedAt: "2026-08-19T00:00:00Z",
      latestProgress: null,
      progress: [],
      links: [],
    };
    let followUpItems = [followUpItem];
    function toolData(request: { domain?: string; action?: string }) {
      const key = `${request.domain}:${request.action}`;
      if (key === "settings:get_all") return { inbox_capture_consent_ack: "true" };
      if (key === "settings:is_autostart_enabled") return { enabled: false };
      if (key === "todo:type_list") return { items: [] };
      if (key === "todo:assignee_list")
        return { items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] };
      if (key === "todo:item_list") return { items: todoItems };
      if (key === "follow_up:item_list") return followUpItems;
      if (key === "pm:project_list") return [];
      if (key === "action_center:definition_list") return { definitions: [] };
      if (key === "usage:tool_summaries" || key === "usage:summaries") return [];
      return {};
    }
    Object.assign(window, {
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener: (id: number) => callbacks.delete(id),
      },
      __TAURI_INTERNALS__: {
        metadata: {
          currentWindow: { label: "main" },
          currentWebview: { windowLabel: "main", label: "main" },
        },
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (command === "tool_execute") {
            const request = args.request as { domain?: string; action?: string };
            return { ok: true, data: toolData(request) };
          }
          if (command === "plugin:event|listen") return args.handler;
          if (command === "plugin:event|unlisten") return null;
          return null;
        },
        transformCallback: (callback: (data: unknown) => void, once = false) => {
          const id = callbackId++;
          callbacks.set(id, (data) => {
            callback(data);
            if (once) callbacks.delete(id);
          });
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        runCallback: (id: number, data: unknown) => callbacks.get(id)?.(data),
        callbacks,
      },
      __setTaskWorkspaceFollowUpCount: (count: number) => {
        followUpItems = Array.from({ length: count }, (_, index) => ({
          ...followUpItem,
          id: index + 1,
          title: `${followUpItem.title} ${index + 1}`,
        }));
      },
      __setTaskWorkspaceTodoCount: (count: number) => {
        todoItems = Array.from({ length: count }, (_, index) => ({
          ...todoItem,
          id: index + 1,
          rootId: index + 1,
          title: `${todoItem.title} ${index + 1}`,
        }));
      },
    });
  });
}

test("keeps the shared mode and badge anchors stable across responsive widths", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 860 });
  for (const width of [1600, 1400, 1200, 1000, 800, 700]) {
    await page.setViewportSize({ width, height: 860 });
    await openTaskWorkspace(page);
    const todoSwitch = await page.locator(".todo-sidebar .task-view-switch").boundingBox();
    const todoList = await page.locator(".todo-list-pane").boundingBox();
    await page
      .locator(".todo-sidebar")
      .getByRole("button", { name: /关注事项/ })
      .click();
    await expect(page.locator(".follow-up-panel")).toBeVisible();
    const followUpSwitch = await page.locator(".follow-up-sidebar .task-view-switch").boundingBox();
    const followUpList = await page.locator(".follow-up-list-pane").boundingBox();
    expect(todoSwitch).not.toBeNull();
    expect(todoList).not.toBeNull();
    expect(followUpSwitch).not.toBeNull();
    expect(followUpList).not.toBeNull();
    expectNear(followUpSwitch!.x, todoSwitch!.x);
    expectNear(followUpSwitch!.y, todoSwitch!.y);
    expectNear(followUpList!.x, todoList!.x);
    expectNear(followUpList!.y, todoList!.y);
  }
});

test("reserves the review badge slot while the follow-up list grows", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openTaskWorkspace(page);
  await page
    .locator(".todo-sidebar")
    .getByRole("button", { name: /关注事项/ })
    .click();
  const switchControl = page.locator(".follow-up-sidebar .task-view-switch");
  const label = switchControl.locator("button").nth(1).locator("span").first();
  const initialLabel = await label.boundingBox();
  const initialSwitch = await switchControl.boundingBox();
  await page.evaluate(() => {
    (
      window as typeof window & { __setTaskWorkspaceFollowUpCount: (count: number) => void }
    ).__setTaskWorkspaceFollowUpCount(100);
  });
  await page.locator('.follow-up-toolbar button[title="刷新"]').click();
  await expect(switchControl.locator(".due-count")).toHaveText("99+");
  const fullLabel = await label.boundingBox();
  const fullSwitch = await switchControl.boundingBox();
  expect(initialLabel).not.toBeNull();
  expect(initialSwitch).not.toBeNull();
  expect(fullLabel).not.toBeNull();
  expect(fullSwitch).not.toBeNull();
  expectNear(fullLabel!.x, initialLabel!.x);
  expectNear(fullSwitch!.width, initialSwitch!.width);

  const listScroll = page.locator(".follow-up-scroll");
  expect(await listScroll.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(
    true,
  );
  await page.evaluate(() => {
    (
      window as typeof window & { __setTaskWorkspaceFollowUpCount: (count: number) => void }
    ).__setTaskWorkspaceFollowUpCount(0);
  });
  await page.locator('.follow-up-toolbar button[title="刷新"]').click();
  await expect(switchControl.locator(".due-count")).toBeHidden();
  const emptyLabel = await label.boundingBox();
  expect(emptyLabel).not.toBeNull();
  expectNear(emptyLabel!.x, initialLabel!.x);
});

test("keeps the todo list independently scrollable while items grow", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 760 });
  await openTaskWorkspace(page);
  await page.evaluate(() => {
    (
      window as typeof window & { __setTaskWorkspaceTodoCount: (count: number) => void }
    ).__setTaskWorkspaceTodoCount(80);
  });
  await page.locator(".todo-list-pane .toolbar-right > .el-button").first().click();

  const listScroll = page.locator(".todo-list-scroll");
  await expect(listScroll.locator(".todo-card")).toHaveCount(80);
  const metrics = await listScroll.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY,
  }));
  expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight);
  expect(metrics.overflowY).toBe("auto");

  await listScroll.hover();
  await page.mouse.wheel(0, 500);
  await expect.poll(() => listScroll.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
});

async function dismissFirstRunPrompt(page: Page) {
  const dismiss = page.getByRole("button", { name: "暂不启用" });
  if (await dismiss.isVisible({ timeout: 3000 }).catch(() => false)) await dismiss.click();
}

async function openTaskWorkspace(page: Page) {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await installTaskBridge(page);
  await page.goto("/");
  await dismissFirstRunPrompt(page);
  try {
    await expect(page.locator(".home-panel")).toBeVisible({ timeout: 10_000 });
  } catch {
    throw new Error(
      `App did not render. Errors: ${pageErrors.join(" | ")} Body: ${await page.locator("body").innerText()}`,
    );
  }
  await page.locator(".home-tool-card", { hasText: "任务清单" }).first().click();
  await expect(page.locator(".task-list-panel")).toBeVisible();
}

async function geometry(page: Page, prefix: "todo" | "follow-up"): Promise<WorkspaceGeometry> {
  const selectors = {
    switchBox: `.${prefix}-sidebar .task-view-switch`,
    sidebarBox: `.${prefix}-sidebar`,
    listBox: `.${prefix}-list-pane`,
    detailBox: `.${prefix}-detail-pane`,
    toolbarBox: `.${prefix}-list-pane .task-workspace-toolbar`,
    listContentBox: `.${prefix}-list-pane .task-workspace-list-content`,
  } as const;
  const entries = await Promise.all(
    Object.entries(selectors).map(async ([key, selector]) => {
      const box = await page.locator(selector).boundingBox();
      if (!box) throw new Error(`${selector} is not visible`);
      return [key, box] as const;
    }),
  );
  return Object.fromEntries(entries) as unknown as WorkspaceGeometry;
}

function expectNear(actual: number, expected: number, tolerance = 2) {
  expect(Math.abs(actual - expected)).toBeLessThanOrEqual(tolerance);
}

test("keeps the task workspace anchors stable while switching views", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openTaskWorkspace(page);

  const todo = await geometry(page, "todo");
  await page
    .locator(".todo-sidebar")
    .getByRole("button", { name: /关注事项/ })
    .click();
  await expect(page.locator(".follow-up-panel")).toBeVisible();
  const followUp = await geometry(page, "follow-up");

  for (const key of ["switchBox", "sidebarBox", "listBox", "detailBox"] as const) {
    expectNear(followUp[key].x, todo[key].x);
    expectNear(followUp[key].y, todo[key].y);
    expectNear(followUp[key].width, todo[key].width);
    expectNear(followUp[key].height, todo[key].height);
  }
  expectNear(
    followUp.toolbarBox.y + followUp.toolbarBox.height,
    todo.toolbarBox.y + todo.toolbarBox.height,
  );
  expectNear(followUp.listContentBox.y, todo.listContentBox.y);
});

for (const viewport of [
  { name: "medium", width: 960, height: 760 },
  { name: "narrow", width: 700, height: 760 },
]) {
  test(`keeps switching and detail navigation reachable at ${viewport.name} width`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await openTaskWorkspace(page);

    const todoSwitch = page.locator(".todo-sidebar .task-view-switch");
    const todoList = page.locator(".todo-list-pane");
    await expect(todoSwitch).toBeVisible();
    await expect(todoList).toBeVisible();
    const todoSwitchBox = await todoSwitch.boundingBox();
    const todoListBox = await todoList.boundingBox();

    await page.locator(".todo-card").first().click();
    const todoDetail = page.locator(".todo-detail-pane");
    await expect(todoDetail).toBeVisible();
    await todoDetail.getByRole("button", { name: "返回列表" }).click();
    await expect(todoDetail).toBeHidden();
    await expect(todoList).toBeVisible();
    await expect(todoSwitch).toBeVisible();

    await todoSwitch.getByRole("button", { name: /关注事项/ }).click();
    const followUpSwitch = page.locator(".follow-up-sidebar .task-view-switch");
    const followUpList = page.locator(".follow-up-list-pane");
    await expect(followUpSwitch).toBeVisible();
    await expect(followUpList).toBeVisible();

    const followUpSwitchBox = await followUpSwitch.boundingBox();
    const followUpListBox = await followUpList.boundingBox();
    if (!todoSwitchBox || !followUpSwitchBox || !todoListBox || !followUpListBox) {
      throw new Error("responsive workspace anchors are not visible");
    }
    expectNear(followUpSwitchBox.x, todoSwitchBox.x);
    expectNear(followUpSwitchBox.y, todoSwitchBox.y);
    expectNear(followUpListBox.x, todoListBox.x);
    expectNear(followUpListBox.y, todoListBox.y);

    await page.locator(".follow-up-card").first().click();
    const detail = page.locator(".follow-up-detail-pane");
    await expect(detail).toBeVisible();
    await detail.getByRole("button", { name: "返回列表" }).click();
    await expect(followUpList).toBeVisible();
    await expect(detail).toBeHidden();
    await expect(followUpSwitch).toBeVisible();
  });
}
