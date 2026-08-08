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
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MergedHomeTool } from "../composables/useFavorites";
import type { SidebarItem } from "../types";
import type { TabItem } from "../types/tabs";
import HomePanel from "./HomePanel.vue";
import TabPageCache from "./TabPageCache.vue";

const tabs: TabItem[] = [
  { id: "first", name: "First", pinned: false },
  { id: "second", name: "Second", pinned: false },
];

let app: ReturnType<typeof createApp> | null = null;
let root: HTMLDivElement | null = null;
const originalMatchMedia = window.matchMedia;

afterEach(() => {
  app?.unmount();
  app = null;
  root?.remove();
  root = null;
  window.matchMedia = originalMatchMedia;
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

function createHomeTool(id: string): MergedHomeTool {
  return {
    tool: { id, name: id, desc: `${id} description` },
    isFavorite: false,
    count: 0,
  };
}

function mountHomeCache(options: { reducedMotion?: boolean } = {}) {
  const activeId = ref("home");
  const mergedHomeTools = ref([createHomeTool("first-tool")]);
  const openTabs: TabItem[] = [
    { id: "home", name: "Home", pinned: true },
    { id: "tool", name: "Tool", pinned: false },
  ];
  const allItems: SidebarItem[] = [];
  const mediaQuery = {
    matches: options.reducedMotion ?? false,
    media: "(prefers-reduced-motion: reduce)",
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList;
  window.matchMedia = vi.fn(() => mediaQuery);

  const host = defineComponent({
    setup() {
      return () =>
        h(
          TabPageCache,
          { tabs: openTabs, activeId: activeId.value },
          {
            default: ({ tab }: { tab: TabItem }) =>
              tab.id === "home"
                ? h(HomePanel, {
                    allItems,
                    mergedHomeTools: mergedHomeTools.value,
                    isFavorite: () => false,
                  })
                : h("div", { "data-tab": "tool" }, "tool"),
          },
        );
    },
  });

  root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp(host);
  app.component(
    "el-button",
    defineComponent({
      setup(_, { attrs, slots }) {
        return () => h("button", attrs, slots.default?.());
      },
    }),
  );
  app.component(
    "el-empty",
    defineComponent({
      props: { description: { type: String, default: "" } },
      setup(props) {
        return () => h("div", props.description);
      },
    }),
  );
  app.mount(root);

  return { activeId, mergedHomeTools };
}

function getHomeCards() {
  return [...document.querySelectorAll<HTMLElement>(".home-tool-card")];
}

function finishHomeCardEntry(card: HTMLElement) {
  const event = new Event("animationend");
  Object.defineProperty(event, "animationName", { value: "cardReveal" });
  card.dispatchEvent(event);
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

  it("restores an interrupted home entry immediately after cached activation", async () => {
    const { activeId } = mountHomeCache();
    await nextTick();

    expect(getHomeCards()).toHaveLength(1);
    expect(getHomeCards()[0].classList.contains("is-home-entry")).toBe(true);

    activeId.value = "tool";
    await nextTick();
    activeId.value = "home";
    await nextTick();

    expect(getHomeCards()[0].classList.contains("is-home-entry")).toBe(false);
  });

  it("does not animate home cards added after first presentation", async () => {
    const { mergedHomeTools } = mountHomeCache();
    await nextTick();

    finishHomeCardEntry(getHomeCards()[0]);
    await nextTick();
    expect(getHomeCards()[0].classList.contains("is-home-entry")).toBe(false);

    mergedHomeTools.value.push(createHomeTool("second-tool"));
    await nextTick();

    expect(getHomeCards()).toHaveLength(2);
    expect(getHomeCards().every((card) => !card.classList.contains("is-home-entry"))).toBe(true);
  });

  it("shows home cards added during first presentation without delaying them", async () => {
    const { mergedHomeTools } = mountHomeCache();
    await nextTick();

    mergedHomeTools.value.push(createHomeTool("second-tool"));
    await nextTick();

    expect(getHomeCards()).toHaveLength(2);
    expect(getHomeCards().every((card) => !card.classList.contains("is-home-entry"))).toBe(true);
  });

  it("ends first presentation when existing home content changes", async () => {
    const { mergedHomeTools } = mountHomeCache();
    await nextTick();

    mergedHomeTools.value[0].tool.name = "renamed-tool";
    await nextTick();

    expect(getHomeCards()[0].classList.contains("is-home-entry")).toBe(false);
  });

  it("shows home cards changed while inactive immediately after restoration", async () => {
    const { activeId, mergedHomeTools } = mountHomeCache();
    await nextTick();

    activeId.value = "tool";
    await nextTick();
    mergedHomeTools.value.push(createHomeTool("second-tool"));
    await nextTick();
    activeId.value = "home";
    await nextTick();

    expect(getHomeCards()).toHaveLength(2);
    expect(getHomeCards().every((card) => !card.classList.contains("is-home-entry"))).toBe(true);
  });

  it("skips the first home entry animation when reduced motion is preferred", async () => {
    mountHomeCache({ reducedMotion: true });
    await nextTick();

    expect(getHomeCards()[0].classList.contains("is-home-entry")).toBe(false);
  });
});
