import { describe, expect, it } from "vitest";
import { nextSpotlightActiveIndex } from "./spotlight-active-index";

describe("nextSpotlightActiveIndex", () => {
  it("resets to the first result when the query changes even if the old index is still valid", () => {
    expect(
      nextSpotlightActiveIndex({
        currentIndex: 7,
        resultCount: 9,
        queryChanged: true,
      }),
    ).toBe(0);
  });

  it("keeps a valid index when results refresh without a query change", () => {
    expect(
      nextSpotlightActiveIndex({
        currentIndex: 3,
        resultCount: 9,
        queryChanged: false,
      }),
    ).toBe(3);
  });

  it("resets when the current index is outside the result list", () => {
    expect(
      nextSpotlightActiveIndex({
        currentIndex: 8,
        resultCount: 3,
        queryChanged: false,
      }),
    ).toBe(0);
  });
});
