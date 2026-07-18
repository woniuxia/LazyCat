import { describe, expect, it } from "vitest";
import {
  addNetworkFavorite,
  buildNetworkFavorite,
  buildNetworkFavoriteFromHistory,
  favoriteToNetworkForm,
  hasNetworkFavorite,
  normalizeNetworkFavorites,
} from "./networkFavorites";

describe("networkFavorites", () => {
  it("builds a named TCP favorite with a host and port target", () => {
    const favorite = buildNetworkFavorite(
      { protocol: "tcp", host: " example.com ", port: 443, timeoutMs: 3000 },
      " 生产 HTTPS ",
      { id: "fav-1", now: 1000 },
    );

    expect(favorite).toEqual({
      id: "fav-1",
      name: "生产 HTTPS",
      protocol: "tcp",
      host: "example.com",
      port: 443,
      timeoutMs: 3000,
      createdAt: 1000,
    });
  });

  it("builds a ping favorite without carrying a port", () => {
    const favorite = buildNetworkFavorite(
      { protocol: "ping", host: "10.0.0.8", port: 3306, timeoutMs: 1000 },
      "",
      { id: "fav-2", now: 2000 },
    );

    expect(favorite.name).toBe("PING 10.0.0.8");
    expect(favorite.port).toBeNull();
  });

  it("rejects invalid TCP or UDP ports", () => {
    expect(() =>
      buildNetworkFavorite(
        { protocol: "udp", host: "127.0.0.1", port: 70000, timeoutMs: 2000 },
        "bad",
        { id: "fav-3", now: 3000 },
      ),
    ).toThrow("端口范围必须是 1-65535");
  });

  it("normalizes persisted favorites and drops invalid rows", () => {
    expect(
      normalizeNetworkFavorites([
        {
          id: "ok",
          name: "Redis",
          protocol: "tcp",
          host: "127.0.0.1",
          port: 6379,
          timeoutMs: 2000,
          createdAt: 1,
        },
        {
          id: "bad-port",
          name: "Bad",
          protocol: "tcp",
          host: "127.0.0.1",
          port: 0,
          timeoutMs: 2000,
          createdAt: 2,
        },
        null,
      ]),
    ).toEqual([
      {
        id: "ok",
        name: "Redis",
        protocol: "tcp",
        host: "127.0.0.1",
        port: 6379,
        timeoutMs: 2000,
        createdAt: 1,
      },
    ]);
  });

  it("moves a repeated target to the front and replaces its name", () => {
    const existing = buildNetworkFavorite(
      { protocol: "tcp", host: "127.0.0.1", port: 6379, timeoutMs: 2000 },
      "Redis old",
      { id: "old", now: 1 },
    );
    const next = buildNetworkFavorite(
      { protocol: "tcp", host: "127.0.0.1", port: 6379, timeoutMs: 2000 },
      "Redis new",
      { id: "new", now: 2 },
    );

    expect(addNetworkFavorite([existing], next)).toEqual([next]);
  });

  it("converts a favorite back to form state", () => {
    const favorite = buildNetworkFavorite(
      { protocol: "tcp", host: "db.internal", port: 3306, timeoutMs: 5000 },
      "Database",
      { id: "fav-4", now: 4000 },
    );

    expect(favoriteToNetworkForm(favorite)).toEqual({
      protocol: "tcp",
      host: "db.internal",
      port: 3306,
      timeoutMs: 5000,
    });
  });

  it("builds a favorite draft from a TCP history row", () => {
    const favorite = buildNetworkFavoriteFromHistory(
      { protocol: "tcp", target: "db.internal:3306", timeoutMs: 5000 },
      "Database",
      { id: "fav-5", now: 5000 },
    );

    expect(favorite).toMatchObject({
      id: "fav-5",
      name: "Database",
      protocol: "tcp",
      host: "db.internal",
      port: 3306,
      timeoutMs: 5000,
    });
  });

  it("detects whether a history target has already been favorited", () => {
    const favorite = buildNetworkFavorite(
      { protocol: "tcp", host: "db.internal", port: 3306, timeoutMs: 5000 },
      "Database",
      { id: "fav-6", now: 6000 },
    );

    expect(
      hasNetworkFavorite([favorite], {
        protocol: "tcp",
        target: "db.internal:3306",
        timeoutMs: 5000,
      }),
    ).toBe(true);
    expect(
      hasNetworkFavorite([favorite], {
        protocol: "tcp",
        target: "db.internal:3306",
        timeoutMs: 2000,
      }),
    ).toBe(false);
  });
  it("parses bracketed IPv6 history targets without splitting the address", () => {
    const favorite = buildNetworkFavoriteFromHistory(
      { protocol: "tcp", target: "[2001:db8::1]:443", timeoutMs: 2000 },
      "IPv6",
      { id: "ipv6", now: 7 },
    );
    expect(favorite.host).toBe("2001:db8::1");
    expect(favorite.port).toBe(443);
  });
});
