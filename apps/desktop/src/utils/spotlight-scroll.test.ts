import { describe, expect, it } from "vitest";
import { nextSpotlightScrollTop } from "./spotlight-scroll";

describe("nextSpotlightScrollTop", () => {
  it("keeps the container position when the item is visible", () => {
    expect(
      nextSpotlightScrollTop({
        scrollTop: 100,
        viewportHeight: 240,
        itemTop: 180,
        itemHeight: 48,
      }),
    ).toBe(100);
  });

  it("scrolls only the result container down when the item is below it", () => {
    expect(
      nextSpotlightScrollTop({
        scrollTop: 100,
        viewportHeight: 240,
        itemTop: 330,
        itemHeight: 48,
      }),
    ).toBe(138);
  });

  it("scrolls the result container up when the item is above it", () => {
    expect(
      nextSpotlightScrollTop({
        scrollTop: 100,
        viewportHeight: 240,
        itemTop: 72,
        itemHeight: 48,
      }),
    ).toBe(72);
  });
});
