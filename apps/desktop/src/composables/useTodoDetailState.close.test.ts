/** @vitest-environment happy-dom */
import { createApp, defineComponent, reactive, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { TodoItem } from "../types";
import { useTodoDetailState, type TodoDetailStateDeps } from "./useTodoDetailState";
import type { TodoItemDraft } from "./useTodoScheduleFields";

function createDraft(): TodoItemDraft {
  return {
    id: 0,
    rootId: 0,
    title: "",
    typeId: undefined,
    priority: "P2",
    description: "",
    assigneeIds: [],
    links: [],
    eventDate: "",
    eventTime: "",
    reminderPresets: ["none"],
    repeatPreset: "none",
    ruleMode: "simple",
    timezone: "local",
    cronExpression: "0 0 9 * * Mon-Fri",
    endMode: "never",
    endValueDate: "",
    endValueCount: 1,
    simple: { frequency: "daily", interval: 1, time: "", weekdays: [1, 2, 3, 4, 5], dayOfMonth: 1 },
    projectId: null,
    pmItemId: null,
    pmItemTitle: null,
    pmItemProjectId: null,
    pmItemStatus: null,
    actionType: null,
    actionTargetId: null,
  };
}

function mountState(submitItemChanges: TodoDetailStateDeps["submitItemChanges"]) {
  const detailMode = ref<"empty" | "view" | "edit" | "create">("edit");
  const selectedItemId = ref<number | null>(1);
  const itemDraft = reactive(createDraft());
  itemDraft.title = "未保存修改";
  const draftBaseline = ref("{}");
  let state!: ReturnType<typeof useTodoDetailState>;
  const component = defineComponent({
    setup() {
      state = useTodoDetailState({
        items: ref([] as TodoItem[]),
        itemDraft,
        detailMode,
        itemDialogMode: ref<"create" | "edit_item">("create"),
        selectedItemId,
        draftBaseline,
        editingItemSnapshot: ref(null),
        showMoreFields: ref(false),
        lastReminderPresetSelection: ref(["none"]),
        defaultReminderPresets: ["none"],
        todoDetailEditRef: ref(null),
        todoPmLinkItemId: ref(null),
        todoPmCandidates: ref([]),
        todoLinkedPmItem: ref(null),
        skipProjectWatch: ref(false),
        loadTodoPmCandidates: async () => {},
        submitItemChanges,
        syncSimpleDraftFromRule: () => {},
        itemKindOf: () => "one_off",
        hasRepeatRule: () => false,
        getItemRecurrence: () => null,
      });
      return () => null;
    },
  });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const app = createApp(component);
  app.mount(host);
  return { app, state, detailMode, selectedItemId };
}

describe("useTodoDetailState closeDetail", () => {
  it("keeps a dirty detail open when saving before close fails", async () => {
    const submit = vi.fn(async () => ({ ok: false, id: null }));
    const mounted = mountState(submit);

    await mounted.state.closeDetail();

    expect(submit).toHaveBeenCalledWith(false);
    expect(mounted.detailMode.value).toBe("edit");
    expect(mounted.selectedItemId.value).toBe(1);
    mounted.app.unmount();
  });

  it("saves a dirty detail and clears selection when close succeeds", async () => {
    const submit = vi.fn(async () => ({ ok: true, id: 1 }));
    const mounted = mountState(submit);

    await mounted.state.closeDetail();

    expect(submit).toHaveBeenCalledWith(false);
    expect(mounted.detailMode.value).toBe("empty");
    expect(mounted.selectedItemId.value).toBeNull();
    mounted.app.unmount();
  });
});
