/** @vitest-environment happy-dom */

import {
  createApp,
  defineComponent,
  h,
  nextTick,
  onActivated,
  onDeactivated,
  onMounted,
  onUnmounted,
  ref,
} from "vue";
import { afterEach, describe, expect, it } from "vitest";
import type { TabItem } from "../types/tabs";
import TabPageCache from "./TabPageCache.vue";

const tabs: TabItem[] = [
  { id: "first", name: "First", pinned: false },
  { id: "second", name: "Second", pinned: false },
];

let app: ReturnType<typeof createApp> | null = null;
let root: HTMLDivElement | null = null;

afterEach(() => {
  app?.unmount();
  app = null;
  root?.remove();
  root = null;
});

function mountCache() {
  const activeId = ref("first");
  const openTabs = ref([...tabs]);
  const lifecycle = {
    firstMounted: 0,
    firstUnmounted: 0,
    firstActivated: 0,
    firstDeactivated: 0,
    secondMounted: 0,
    secondUnmounted: 0,
    secondActivated: 0,
    secondDeactivated: 0,
  };

  const panels = new Map(
    tabs.map((tab) => [
      tab.id,
      defineComponent({
        setup() {
          const value = ref(`${tab.id}-initial`);
          onMounted(() => {
            lifecycle[`${tab.id}Mounted` as keyof typeof lifecycle] += 1;
          });
          onUnmounted(() => {
            lifecycle[`${tab.id}Unmounted` as keyof typeof lifecycle] += 1;
          });
          onActivated(() => {
            lifecycle[`${tab.id}Activated` as keyof typeof lifecycle] += 1;
          });
          onDeactivated(() => {
            lifecycle[`${tab.id}Deactivated` as keyof typeof lifecycle] += 1;
          });
          return () =>
            h(
              "button",
              {
                "data-tab": tab.id,
                onClick: () => {
                  value.value = `${tab.id}-edited`;
                },
              },
              value.value,
            );
        },
      }),
    ]),
  );

  const host = defineComponent({
    setup() {
      return () =>
        h(
          TabPageCache,
          { tabs: openTabs.value, activeId: activeId.value },
          {
            default: ({ tab }: { tab: TabItem }) => {
              const Panel = panels.get(tab.id);
              return Panel ? h(Panel) : null;
            },
          },
        );
    },
  });

  root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp(host);
  app.mount(root);

  return { activeId, openTabs, lifecycle };
}

describe("TabPageCache", () => {
  it("keeps a page instance when switching tabs", async () => {
    const { activeId, lifecycle } = mountCache();
    await nextTick();

    const firstScroll = document.querySelector('[data-tab-scroll-id="first"]') as HTMLElement;
    firstScroll.scrollTop = 240;
    (document.querySelector('[data-tab="first"]') as HTMLButtonElement).click();
    await nextTick();
    activeId.value = "second";
    await nextTick();
    const secondScroll = document.querySelector('[data-tab-scroll-id="second"]') as HTMLElement;
    expect(document.querySelector('[data-tab-scroll-id="first"]')).toBe(firstScroll);
    expect(document.querySelector('[data-tab="first"]')).toBeNull();
    secondScroll.scrollTop = 80;
    activeId.value = "first";
    await nextTick();

    expect(document.querySelector('[data-tab="first"]')?.textContent).toBe("first-edited");
    expect(document.querySelector('[data-tab-scroll-id="first"]')).toBe(firstScroll);
    expect((document.querySelector('[data-tab-scroll-id="first"]') as HTMLElement).scrollTop).toBe(
      240,
    );
    expect(lifecycle.firstMounted).toBe(1);
    expect(lifecycle.firstUnmounted).toBe(0);
    expect(lifecycle.firstActivated).toBe(2);
    expect(lifecycle.firstDeactivated).toBe(1);
    expect(lifecycle.secondMounted).toBe(1);
    expect(lifecycle.secondUnmounted).toBe(0);
    expect(lifecycle.secondActivated).toBe(1);
    expect(lifecycle.secondDeactivated).toBe(1);
  });

  it("destroys the cache when a tab is removed", async () => {
    const { activeId, openTabs, lifecycle } = mountCache();
    await nextTick();

    activeId.value = "second";
    await nextTick();
    openTabs.value = openTabs.value.filter((tab) => tab.id !== "first");
    await nextTick();

    expect(lifecycle.firstMounted).toBe(1);
    expect(lifecycle.firstUnmounted).toBe(1);
    expect(document.querySelector('[data-tab="first"]')).toBeNull();
  });

  it("destroys every page instance when all tabs are removed", async () => {
    const { activeId, openTabs, lifecycle } = mountCache();
    await nextTick();

    activeId.value = "second";
    await nextTick();
    openTabs.value = [];
    await nextTick();

    expect(lifecycle.firstUnmounted).toBe(1);
    expect(lifecycle.secondUnmounted).toBe(1);
    expect(document.querySelector("[data-tab-scroll-id]")).toBeNull();
  });
});
