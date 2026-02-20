/**
 * Type-safe localStorage read/write helpers.
 */
export function loadJson<T>(key: string, fallback: T, validate?: (v: unknown) => v is T): T {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const parsed: unknown = JSON.parse(raw);
    if (validate) {
      return validate(parsed) ? parsed : fallback;
    }
    return parsed as T;
  } catch {
    return fallback;
  }
}

export function saveJson(key: string, value: unknown): void {
  localStorage.setItem(key, JSON.stringify(value));
}

export function loadString(key: string, fallback: string = ""): string {
  return localStorage.getItem(key) ?? fallback;
}

export function saveString(key: string, value: string): void {
  localStorage.setItem(key, value);
}
