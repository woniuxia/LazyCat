export interface SpotlightScrollInput {
  scrollTop: number;
  viewportHeight: number;
  itemTop: number;
  itemHeight: number;
}

export function nextSpotlightScrollTop({
  scrollTop,
  viewportHeight,
  itemTop,
  itemHeight,
}: SpotlightScrollInput): number {
  const currentScrollTop = Math.max(0, scrollTop);
  const viewportBottom = currentScrollTop + Math.max(0, viewportHeight);
  const itemBottom = itemTop + Math.max(0, itemHeight);

  if (itemTop < currentScrollTop) return Math.max(0, itemTop);
  if (itemBottom > viewportBottom) {
    return Math.max(0, itemBottom - Math.max(0, viewportHeight));
  }
  return currentScrollTop;
}
