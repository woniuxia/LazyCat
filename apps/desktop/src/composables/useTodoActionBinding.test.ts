import { beforeEach, describe, expect, it, vi } from "vitest";
import { reactive } from "vue";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));

import { useTodoActionBinding } from "./useTodoActionBinding";

function createDraft() {
  return reactive({
    actionType: null as string | null,
    actionTargetId: null as string | null,
  });
}

describe("useTodoActionBinding", () => {
  beforeEach(() => invokeMock.mockReset());

  it("loads definitions, then loads targets when an action is selected", async () => {
    invokeMock
      .mockResolvedValueOnce({
        definitions: [
          {
            actionType: "release_package.run",
            label: "开始打包",
            triggerTypes: ["todo_item"],
            targetKind: "release_package_project",
            targetToolId: "release-package",
            executionMode: "open_and_confirm",
            completionPolicy: "on_succeeded",
          },
        ],
      })
      .mockResolvedValueOnce({
        targets: [{ id: "7", label: "客户门户", available: true }],
      });
    const draft = createDraft();
    const binding = useTodoActionBinding(draft);

    await binding.loadDefinitions();
    await binding.onActionTypeChange("release_package.run");

    expect(binding.actionDefinitions.value).toHaveLength(1);
    expect(draft.actionType).toBe("release_package.run");
    expect(draft.actionTargetId).toBeNull();
    expect(invokeMock).toHaveBeenNthCalledWith(1, "tool:action-center:definition-list", {});
    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:action-center:target-list", {
      actionType: "release_package.run",
    });
    expect(binding.actionTargets.value).toEqual([
      { id: "7", label: "客户门户", available: true },
    ]);
  });

  it("clears the target when the action is cleared", async () => {
    const draft = createDraft();
    draft.actionType = "release_package.run";
    draft.actionTargetId = "7";
    const binding = useTodoActionBinding(draft);

    await binding.onActionTypeChange(null);

    expect(draft.actionType).toBeNull();
    expect(draft.actionTargetId).toBeNull();
    expect(binding.actionTargets.value).toEqual([]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("dispatches a Todo action with a string trigger id", async () => {
    invokeMock.mockResolvedValueOnce({
      id: "dispatch-1",
      triggerType: "todo_item",
      triggerId: "42",
      actionType: "release_package.run",
      targetId: "7",
      status: "pending_confirmation",
      createdAt: "2026-07-25T10:00:00Z",
    });
    const binding = useTodoActionBinding(createDraft());

    const result = await binding.dispatchTodoAction({ id: 42 }, { triggerEventId: undefined });

    expect(invokeMock).toHaveBeenCalledWith("tool:action-center:dispatch", {
      triggerType: "todo_item",
      triggerId: "42",
    });
    expect(result.id).toBe("dispatch-1");
    expect(binding.latestDispatch.value?.status).toBe("pending_confirmation");
  });

  it("queries the latest dispatch with the Todo trigger type", async () => {
    invokeMock.mockResolvedValueOnce({ dispatch: null });
    const binding = useTodoActionBinding(createDraft());

    await binding.loadLatestDispatch(42);

    expect(invokeMock).toHaveBeenCalledWith("tool:action-center:dispatch-latest", {
      triggerType: "todo_item",
      triggerId: "42",
    });
    expect(binding.latestDispatch.value).toBeNull();
  });

  it("does not let an older target response replace the latest action targets", async () => {
    let resolveFirst!: (value: unknown) => void;
    invokeMock
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockResolvedValueOnce({
        targets: [{ id: "8", label: "浏览器身份", available: true }],
      });
    const draft = createDraft();
    const binding = useTodoActionBinding(draft);

    const first = binding.onActionTypeChange("release_package.run");
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    await binding.onActionTypeChange("browser_profile.open");
    resolveFirst({ targets: [{ id: "7", label: "客户门户", available: true }] });
    await first;

    expect(draft.actionType).toBe("browser_profile.open");
    expect(binding.actionTargets.value).toEqual([
      { id: "8", label: "浏览器身份", available: true },
    ]);
    expect(binding.isAvailableTarget("browser_profile.open", "8")).toBe(true);
    expect(binding.isAvailableTarget("release_package.run", "8")).toBe(false);
  });

  it("does not restore a stale dispatch after the selected Todo is cleared", async () => {
    let resolveLatest!: (value: unknown) => void;
    invokeMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveLatest = resolve;
        }),
    );
    const binding = useTodoActionBinding(createDraft());

    const pending = binding.loadLatestDispatch(42);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    binding.clearLatestDispatch();
    resolveLatest({
      dispatch: {
        id: "stale",
        triggerType: "todo_item",
        triggerId: "42",
        actionType: "release_package.run",
        targetId: "7",
        status: "failed",
        createdAt: "2026-07-25T10:00:00Z",
      },
    });
    await pending;

    expect(binding.latestDispatch.value).toBeNull();
  });
});
