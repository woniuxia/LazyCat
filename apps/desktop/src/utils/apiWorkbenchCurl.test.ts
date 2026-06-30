import { describe, expect, it } from "vitest";
import { parseApiWorkbenchCurl } from "./apiWorkbenchCurl";

describe("apiWorkbenchCurl", () => {
  it("parses json post command with headers and body", () => {
    const result = parseApiWorkbenchCurl(
      `curl -X POST http://127.0.0.1:8080/api/users -H "Content-Type: application/json" -d '{"name":"Tom"}'`,
    );

    expect(result.warnings).toEqual([]);
    expect(result.draft.method).toBe("POST");
    expect(result.draft.url).toBe("http://127.0.0.1:8080/api/users");
    expect(result.draft.headers).toEqual([
      { enabled: true, key: "Content-Type", value: "application/json" },
    ]);
    expect(result.draft.bodyType).toBe("json");
    expect(result.draft.body).toBe('{"name":"Tom"}');
  });

  it("splits query string into draft query rows", () => {
    const result = parseApiWorkbenchCurl(
      `curl 'http://127.0.0.1:8080/api/users?page=1&keyword=Tom'`,
    );

    expect(result.draft.url).toBe("http://127.0.0.1:8080/api/users");
    expect(result.draft.query).toEqual([
      { enabled: true, key: "page", value: "1" },
      { enabled: true, key: "keyword", value: "Tom" },
    ]);
  });

  it("treats -G data as query rows", () => {
    const result = parseApiWorkbenchCurl(
      `curl -G http://127.0.0.1:8080/api/users --data 'page=1&keyword=Tom'`,
    );

    expect(result.draft.method).toBe("GET");
    expect(result.draft.bodyType).toBe("none");
    expect(result.draft.query).toEqual([
      { enabled: true, key: "page", value: "1" },
      { enabled: true, key: "keyword", value: "Tom" },
    ]);
  });

  it("rejects file references for data flags but keeps data-raw at literal", () => {
    expect(() =>
      parseApiWorkbenchCurl(`curl http://127.0.0.1:8080/api/users --data @body.json`),
    ).toThrow(/不读取本地文件/);

    const result = parseApiWorkbenchCurl(
      `curl http://127.0.0.1:8080/api/users --data-raw '@body.json'`,
    );
    expect(result.draft.body).toBe("@body.json");
  });
});
