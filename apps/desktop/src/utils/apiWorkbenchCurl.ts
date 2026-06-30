import type { ApiWorkbenchKeyValueRow, ApiWorkbenchRequestDraft } from "../types/api-workbench";
import { normalizeApiWorkbenchDraft } from "./apiWorkbench";

export interface ApiWorkbenchCurlParseResult {
  draft: ApiWorkbenchRequestDraft;
  warnings: string[];
}

type DataPart = {
  flag: string;
  value: string;
};

function tokenizeCurl(input: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];
    if (quote) {
      if (char === quote) {
        quote = null;
        continue;
      }
      if (quote === '"' && char === "\\" && index + 1 < input.length) {
        index += 1;
        current += input[index];
        continue;
      }
      current += char;
      continue;
    }

    if (char === "'" || char === '"') {
      quote = char;
      continue;
    }
    if (/\s/.test(char)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      continue;
    }
    if (char === "\\" && index + 1 < input.length) {
      index += 1;
      current += input[index];
      continue;
    }
    current += char;
  }
  if (quote) throw new Error("cURL 命令引号不完整");
  if (current) tokens.push(current);
  return tokens;
}

function readFlagValue(tokens: string[], index: number, flag: string): { value: string; next: number } {
  const equalIndex = flag.indexOf("=");
  if (equalIndex >= 0) {
    return { value: flag.slice(equalIndex + 1), next: index + 1 };
  }
  const value = tokens[index + 1];
  if (value === undefined) throw new Error(`${flag} 缺少参数值`);
  return { value, next: index + 2 };
}

function parseHeader(value: string): ApiWorkbenchKeyValueRow {
  const colon = value.indexOf(":");
  if (colon <= 0) throw new Error(`Header 格式错误：${value}`);
  return {
    enabled: true,
    key: value.slice(0, colon).trim(),
    value: value.slice(colon + 1).trim(),
  };
}

function appendQueryRows(rows: ApiWorkbenchKeyValueRow[], raw: string) {
  const params = new URLSearchParams(raw);
  for (const [key, value] of params.entries()) {
    rows.push({ enabled: true, key, value });
  }
}

function splitUrlQuery(rawUrl: string): { url: string; query: ApiWorkbenchKeyValueRow[] } {
  const hashIndex = rawUrl.indexOf("#");
  const withoutHash = hashIndex >= 0 ? rawUrl.slice(0, hashIndex) : rawUrl;
  const queryIndex = withoutHash.indexOf("?");
  if (queryIndex < 0) return { url: withoutHash, query: [] };
  const query: ApiWorkbenchKeyValueRow[] = [];
  appendQueryRows(query, withoutHash.slice(queryIndex + 1));
  return { url: withoutHash.slice(0, queryIndex), query };
}

function parseFormData(data: string): ApiWorkbenchKeyValueRow[] | null {
  if (!data.includes("=")) return null;
  const rows: ApiWorkbenchKeyValueRow[] = [];
  appendQueryRows(rows, data);
  return rows.length > 0 ? rows : null;
}

export function parseApiWorkbenchCurl(input: string): ApiWorkbenchCurlParseResult {
  const tokens = tokenizeCurl(input.trim());
  if (tokens.length === 0) throw new Error("请输入 cURL 命令");
  if (tokens[0] !== "curl") throw new Error("命令必须以 curl 开头");

  let method = "";
  let url = "";
  let useDataAsQuery = false;
  const headers: ApiWorkbenchKeyValueRow[] = [];
  const dataParts: DataPart[] = [];

  for (let index = 1; index < tokens.length; ) {
    const token = tokens[index];
    if (token === "-G") {
      useDataAsQuery = true;
      index += 1;
      continue;
    }
    if (token === "-X" || token === "--request" || token.startsWith("--request=")) {
      const read = readFlagValue(tokens, index, token);
      method = read.value.toUpperCase();
      index = read.next;
      continue;
    }
    if (token === "-H" || token === "--header" || token.startsWith("--header=")) {
      const read = readFlagValue(tokens, index, token);
      headers.push(parseHeader(read.value));
      index = read.next;
      continue;
    }
    if (
      token === "-d" ||
      token === "--data" ||
      token === "--data-raw" ||
      token === "--data-binary" ||
      token.startsWith("--data=") ||
      token.startsWith("--data-raw=") ||
      token.startsWith("--data-binary=")
    ) {
      const read = readFlagValue(tokens, index, token);
      const flag = token.split("=")[0];
      if ((flag === "-d" || flag === "--data" || flag === "--data-binary") && read.value.startsWith("@")) {
        throw new Error("第一版 cURL 导入不读取本地文件内容");
      }
      dataParts.push({ flag, value: read.value });
      index = read.next;
      continue;
    }
    if (token === "--url" || token.startsWith("--url=")) {
      const read = readFlagValue(tokens, index, token);
      url = read.value;
      index = read.next;
      continue;
    }
    if (token.startsWith("-")) {
      throw new Error(`暂不支持 cURL 参数：${token}`);
    }
    if (url) throw new Error(`无法解析多余参数：${token}`);
    url = token;
    index += 1;
  }

  if (!url) throw new Error("cURL 命令缺少 URL");
  const split = splitUrlQuery(url);
  const query = [...split.query];
  const data = dataParts.map((part) => part.value).join("&");
  const contentType = headers.find((row) => row.key.toLowerCase() === "content-type")?.value.toLowerCase() ?? "";

  let bodyType: ApiWorkbenchRequestDraft["bodyType"] = "none";
  let body = "";
  let form: ApiWorkbenchKeyValueRow[] = [];
  if (data) {
    if (useDataAsQuery) {
      appendQueryRows(query, data);
    } else if (contentType.includes("application/json")) {
      bodyType = "json";
      body = data;
    } else if (contentType.includes("application/x-www-form-urlencoded")) {
      const parsed = parseFormData(data);
      if (parsed) {
        bodyType = "form-urlencoded";
        form = parsed;
      } else {
        bodyType = "text";
        body = data;
      }
    } else {
      bodyType = "text";
      body = data;
    }
  }

  return {
    warnings: [],
    draft: normalizeApiWorkbenchDraft({
      method: method || (data && !useDataAsQuery ? "POST" : "GET"),
      url: split.url,
      query,
      headers,
      bodyType,
      body,
      form,
      timeoutMs: 10000,
    }),
  };
}
