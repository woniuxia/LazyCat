export interface TodoContextMenuPositionInput {
  anchorX: number;
  anchorY: number;
  menuWidth: number;
  menuHeight: number;
  viewportWidth: number;
  viewportHeight: number;
  padding?: number;
}

export interface TodoContextMenuPosition {
  x: number;
  y: number;
}

export function clampContextMenuPosition(
  input: TodoContextMenuPositionInput,
): TodoContextMenuPosition {
  const padding = Math.max(0, input.padding ?? 12);
  const maxX = Math.max(padding, input.viewportWidth - input.menuWidth - padding);
  const maxY = Math.max(padding, input.viewportHeight - input.menuHeight - padding);
  return {
    x: Math.min(Math.max(input.anchorX, padding), maxX),
    y: Math.min(Math.max(input.anchorY, padding), maxY),
  };
}
