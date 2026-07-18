import { describe, expect, it } from "vitest";
import { AccessPathInputError, normalizeAccessPathInput } from "./accessPathInput";

describe("normalizeAccessPathInput", () => {
  it("uses HTTPS and port 443 for a bare hostname", () => {
    expect(normalizeAccessPathInput(" Example.COM ")).toMatchObject({
      protocol: "https",
      hostname: "example.com",
      targetKind: "hostname",
      port: 443,
      path: "/",
      url: "https://example.com/",
      sni: "example.com",
      verifyHostname: "example.com",
      httpHost: "example.com",
    });
  });

  it("keeps an explicit HTTP port and path", () => {
    expect(normalizeAccessPathInput("http://example.com:8080/health?full=1")).toMatchObject({
      protocol: "http",
      port: 8080,
      path: "/health?full=1",
      url: "http://example.com:8080/health?full=1",
      httpHost: "example.com:8080",
    });
  });

  it("parses bracketed IPv6 with a port without splitting on colons", () => {
    expect(normalizeAccessPathInput("[2001:DB8::1]:8443/status")).toMatchObject({
      hostname: "2001:db8::1",
      targetKind: "ipv6",
      port: 8443,
      sni: null,
      verifyHostname: "2001:db8::1",
      httpHost: "[2001:db8::1]:8443",
      url: "https://[2001:db8::1]:8443/status",
    });
  });

  it("parses an IPv4 address with an explicit port", () => {
    expect(normalizeAccessPathInput("192.0.2.10:8080", { defaultProtocol: "http" })).toMatchObject({
      protocol: "http",
      hostname: "192.0.2.10",
      targetKind: "ipv4",
      port: 8080,
      url: "http://192.0.2.10:8080/",
    });
  });

  it("accepts a bare IPv6 literal without mistaking the last segment for a port", () => {
    expect(normalizeAccessPathInput("2001:db8::8")).toMatchObject({
      hostname: "2001:db8::8",
      targetKind: "ipv6",
      port: 443,
      url: "https://[2001:db8::8]/",
    });
  });

  it("canonicalizes an expanded IPv6 literal", () => {
    expect(normalizeAccessPathInput("2001:0DB8:0:0:0:0:0:1")).toMatchObject({
      hostname: "2001:db8::1",
      targetKind: "ipv6",
      url: "https://[2001:db8::1]/",
    });
  });

  it("keeps SNI, certificate verification, HTTP Host and connection IP independent", () => {
    expect(
      normalizeAccessPathInput("https://service.internal:443/", {
        sni: "public.example.com",
        verifyHostname: "certificate.example.com",
        httpHost: "tenant.example.com",
        connectionIp: "192.0.2.10",
      }),
    ).toMatchObject({
      sni: "public.example.com",
      verifyHostname: "certificate.example.com",
      httpHost: "tenant.example.com",
      connectionIp: "192.0.2.10",
    });
  });

  it("normalizes IPv6 connection and HTTP Host overrides", () => {
    expect(
      normalizeAccessPathInput("example.com", {
        httpHost: "[2001:0DB8:0:0:0:0:0:2]:8443",
        connectionIp: "2001:0DB8:0:0:0:0:0:1",
      }),
    ).toMatchObject({
      httpHost: "[2001:db8::2]:8443",
      connectionIp: "2001:db8::1",
    });
  });

  it("rejects unsupported protocols, credentials and invalid ports", () => {
    expect(() => normalizeAccessPathInput("ftp://example.com")).toThrowError(AccessPathInputError);
    expect(() => normalizeAccessPathInput("https://user:pass@example.com")).toThrow("用户名或密码");
    expect(() => normalizeAccessPathInput("example.com:70000")).toThrow("端口范围必须是 1-65535");
  });

  it("rejects a non-IP connection override", () => {
    expect(() => normalizeAccessPathInput("example.com", { connectionIp: "not-an-ip" })).toThrow(
      "连接 IP 必须是 IPv4 或 IPv6 地址",
    );
  });

  it("rejects an IP or port in SNI", () => {
    expect(() => normalizeAccessPathInput("example.com", { sni: "192.0.2.10" })).toThrow(
      "SNI 必须是主机名",
    );
    expect(() => normalizeAccessPathInput("example.com", { sni: "example.com:443" })).toThrow(
      "主机名格式无效",
    );
  });

  it("accepts a hostname or IP certificate verification override and rejects ports", () => {
    expect(
      normalizeAccessPathInput("example.com", { verifyHostname: "API.Example.COM." }),
    ).toMatchObject({ verifyHostname: "api.example.com" });
    expect(
      normalizeAccessPathInput("example.com", { verifyHostname: "2001:0DB8:0:0::1" }),
    ).toMatchObject({ verifyHostname: "2001:db8::1" });
    expect(() =>
      normalizeAccessPathInput("example.com", { verifyHostname: "api.example.com:443" }),
    ).toThrow("主机名格式无效");
  });

  it("rejects malformed hostnames", () => {
    expect(() => normalizeAccessPathInput("example..com")).toThrow("主机名格式无效");
    expect(() => normalizeAccessPathInput("-api.example.com")).toThrow("主机名格式无效");
  });
});
