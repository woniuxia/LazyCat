export interface SpotlightActiveIndexInput {
  currentIndex: number;
  resultCount: number;
  queryChanged: boolean;
}

export function nextSpotlightActiveIndex({
  currentIndex,
  resultCount,
  queryChanged,
}: SpotlightActiveIndexInput): number {
  if (queryChanged) return 0;
  if (resultCount <= 0) return 0;
  if (currentIndex < 0) return 0;
  if (currentIndex >= resultCount) return 0;
  return currentIndex;
}
