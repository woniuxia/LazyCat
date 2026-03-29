import { describe, expect, it } from "vitest";

import { clampContextMenuPosition } from "./contextMenu";

describe("clampContextMenuPosition", () => {
  it("保留视口内的正常坐标", () => {
    expect(
      clampContextMenuPosition({
        anchorX: 320,
        anchorY: 240,
        menuWidth: 180,
        menuHeight: 140,
        viewportWidth: 1280,
        viewportHeight: 720,
      }),
    ).toEqual({ x: 320, y: 240 });
  });

  it("在右侧和底部空间不足时回退到可见区域", () => {
    expect(
      clampContextMenuPosition({
        anchorX: 1240,
        anchorY: 700,
        menuWidth: 180,
        menuHeight: 140,
        viewportWidth: 1280,
        viewportHeight: 720,
      }),
    ).toEqual({ x: 1088, y: 568 });
  });

  it("保护左上角最小边距", () => {
    expect(
      clampContextMenuPosition({
        anchorX: 3,
        anchorY: 6,
        menuWidth: 180,
        menuHeight: 140,
        viewportWidth: 1280,
        viewportHeight: 720,
      }),
    ).toEqual({ x: 12, y: 12 });
  });

  it("在菜单尺寸超过视口时仍固定到边距内", () => {
    expect(
      clampContextMenuPosition({
        anchorX: 60,
        anchorY: 40,
        menuWidth: 240,
        menuHeight: 180,
        viewportWidth: 160,
        viewportHeight: 120,
      }),
    ).toEqual({ x: 12, y: 12 });
  });
});
