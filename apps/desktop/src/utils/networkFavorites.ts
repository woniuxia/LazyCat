export type NetworkFavoriteProtocol = "tcp" | "udp" | "ping";

export interface NetworkFavoriteForm {
  protocol: NetworkFavoriteProtocol;
  host: string;
  port: number;
  timeoutMs: number;
}

export interface NetworkFavoriteItem {
  id: string;
  name: string;
  protocol: NetworkFavoriteProtocol;
  host: string;
  port: number | null;
  timeoutMs: number;
  createdAt: number;
}

export interface NetworkHistoryFavoriteSource {
  protocol: NetworkFavoriteProtocol;
  target: string;
  timeoutMs: number;
}

interface BuildOptions {
  id?: string;
  now?: number;
}

const MAX_FAVORITES = 30;
const DEFAULT_TIMEOUT_MS = 2000;

function isProtocol(value: unknown): value is NetworkFavoriteProtocol {
  return value === "tcp" || value === "udp" || value === "ping";
}

function normalizePort(value: unknown): number | null {
  const port = Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  return port;
}

function normalizeTimeout(value: unknown): number {
  const timeout = Number(value);
  if (!Number.isFinite(timeout) || timeout < 100) return DEFAULT_TIMEOUT_MS;
  return Math.min(Math.round(timeout), 10000);
}

function buildFavoriteId(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function getNetworkFavoriteLabel(form: NetworkFavoriteForm): string {
  const normalizedHost = form.host.trim();
  if (form.protocol === "tcp" || form.protocol === "udp") {
    return `${form.protocol.toUpperCase()} ${normalizedHost}:${form.port}`;
  }
  return `PING ${normalizedHost}`;
}

export function buildNetworkFavorite(
  form: NetworkFavoriteForm,
  name: string,
  options: BuildOptions = {},
): NetworkFavoriteItem {
  const protocol = form.protocol;
  const host = form.host.trim();
  if (!host) {
    throw new Error("请输入主机地址");
  }

  const port = protocol === "ping" ? null : normalizePort(form.port);
  if (protocol !== "ping" && port === null) {
    throw new Error("端口范围必须是 1-65535");
  }

  const fallbackName = getNetworkFavoriteLabel({ ...form, host, port: port ?? form.port });
  return {
    id: options.id ?? buildFavoriteId(),
    name: name.trim() || fallbackName,
    protocol,
    host,
    port,
    timeoutMs: normalizeTimeout(form.timeoutMs),
    createdAt: options.now ?? Date.now(),
  };
}

export function historySourceToNetworkForm(
  source: NetworkHistoryFavoriteSource,
): NetworkFavoriteForm {
  if (source.protocol === "tcp" || source.protocol === "udp") {
    const separatorIndex = source.target.lastIndexOf(":");
    const host = separatorIndex > 0 ? source.target.slice(0, separatorIndex) : source.target;
    const port = separatorIndex > 0 ? Number(source.target.slice(separatorIndex + 1)) : 0;
    return {
      protocol: source.protocol,
      host,
      port,
      timeoutMs: source.timeoutMs,
    };
  }

  return {
    protocol: source.protocol,
    host: source.target,
    port: 80,
    timeoutMs: source.timeoutMs,
  };
}

export function buildNetworkFavoriteFromHistory(
  source: NetworkHistoryFavoriteSource,
  name: string,
  options: BuildOptions = {},
): NetworkFavoriteItem {
  return buildNetworkFavorite(historySourceToNetworkForm(source), name, options);
}

export function normalizeNetworkFavorites(
  raw: unknown,
  limit = MAX_FAVORITES,
): NetworkFavoriteItem[] {
  if (!Array.isArray(raw)) return [];

  const rows: NetworkFavoriteItem[] = [];
  for (const item of raw) {
    const value = item as Record<string, unknown> | null;
    if (!value || typeof value !== "object") continue;
    if (typeof value.id !== "string" || typeof value.name !== "string") continue;
    if (!isProtocol(value.protocol) || typeof value.host !== "string") continue;
    if (typeof value.createdAt !== "number") continue;

    const host = value.host.trim();
    if (!host) continue;

    const port = value.protocol === "ping" ? null : normalizePort(value.port);
    if (value.protocol !== "ping" && port === null) continue;

    rows.push({
      id: value.id,
      name:
        value.name.trim() ||
        getNetworkFavoriteLabel({
          protocol: value.protocol,
          host,
          port: port ?? 0,
          timeoutMs: normalizeTimeout(value.timeoutMs),
        }),
      protocol: value.protocol,
      host,
      port,
      timeoutMs: normalizeTimeout(value.timeoutMs),
      createdAt: value.createdAt,
    });
  }

  return rows.slice(0, limit);
}

export function isSameNetworkFavoriteTarget(
  a: NetworkFavoriteItem,
  b: NetworkFavoriteItem,
): boolean {
  return (
    a.protocol === b.protocol &&
    a.host.toLowerCase() === b.host.toLowerCase() &&
    a.port === b.port &&
    a.timeoutMs === b.timeoutMs
  );
}

export function addNetworkFavorite(
  favorites: NetworkFavoriteItem[],
  next: NetworkFavoriteItem,
  limit = MAX_FAVORITES,
): NetworkFavoriteItem[] {
  return [next, ...favorites.filter((item) => !isSameNetworkFavoriteTarget(item, next))].slice(
    0,
    limit,
  );
}

export function hasNetworkFavorite(
  favorites: NetworkFavoriteItem[],
  source: NetworkHistoryFavoriteSource,
): boolean {
  const target = buildNetworkFavoriteFromHistory(source, "");
  return favorites.some((item) => isSameNetworkFavoriteTarget(item, target));
}

export function favoriteToNetworkForm(favorite: NetworkFavoriteItem): NetworkFavoriteForm {
  return {
    protocol: favorite.protocol,
    host: favorite.host,
    port: favorite.port ?? 80,
    timeoutMs: favorite.timeoutMs,
  };
}
