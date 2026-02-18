import { invoke } from "@tauri-apps/api/core";

export interface ToolRequest {
  request_id: string;
  domain: string;
  action: string;
  payload: Record<string, unknown>;
}

export interface ToolResponse {
  request_id: string;
  ok: boolean;
  data?: unknown;
  error?: { code: string; message: string; details?: unknown };
  meta?: { duration_ms: number; warnings?: string[] };
}

export async function registerHotkey(shortcut: string): Promise<void> {
  await invoke("register_hotkey", { shortcut });
}

export async function unregisterHotkey(): Promise<void> {
  await invoke("unregister_hotkey");
}

const CHANNEL_MAP: Record<string, { domain: string; action: string }> = {
  "tool:encode:base64-encode": { domain: "encode", action: "base64_encode" },
  "tool:encode:base64-decode": { domain: "encode", action: "base64_decode" },
  "tool:encode:url-encode": { domain: "encode", action: "url_encode" },
  "tool:encode:url-decode": { domain: "encode", action: "url_decode" },
  "tool:encode:md5": { domain: "encode", action: "md5" },
  "tool:encode:qr": { domain: "encode", action: "qr_generate" },
  "tool:convert:json-to-xml": { domain: "convert", action: "json_to_xml" },
  "tool:convert:xml-to-json": { domain: "convert", action: "xml_to_json" },
  "tool:convert:json-to-yaml": { domain: "convert", action: "json_to_yaml" },
  "tool:convert:csv-to-json": { domain: "convert", action: "csv_to_json" },
  "tool:text:unique-lines": { domain: "text", action: "unique_lines" },
  "tool:text:sort-lines": { domain: "text", action: "sort_lines" },
  "tool:time:timestamp-to-date": { domain: "time", action: "timestamp_to_date" },
  "tool:time:date-to-timestamp": { domain: "time", action: "date_to_timestamp" },
  "tool:gen:uuid": { domain: "gen", action: "uuid" },
  "tool:gen:guid": { domain: "gen", action: "guid" },
  "tool:gen:password": { domain: "gen", action: "password" },
  "tool:regex:test": { domain: "regex", action: "test" },
  "tool:regex:generate": { domain: "regex", action: "generate" },
  "tool:regex:templates": { domain: "regex", action: "templates" },
  "tool:cron:generate": { domain: "cron", action: "generate" },
  "tool:cron:preview": { domain: "cron", action: "preview" },
  "tool:crypto:rsa-encrypt": { domain: "crypto", action: "rsa_encrypt" },
  "tool:crypto:rsa-decrypt": { domain: "crypto", action: "rsa_decrypt" },
  "tool:crypto:aes-encrypt": { domain: "crypto", action: "aes_encrypt" },
  "tool:crypto:aes-decrypt": { domain: "crypto", action: "aes_decrypt" },
  "tool:crypto:des-encrypt": { domain: "crypto", action: "des_encrypt" },
  "tool:crypto:des-decrypt": { domain: "crypto", action: "des_decrypt" },
  "tool:format:json": { domain: "format", action: "json" },
  "tool:format:xml": { domain: "format", action: "xml" },
  "tool:format:html": { domain: "format", action: "html" },
  "tool:format:java": { domain: "format", action: "java" },
  "tool:format:sql": { domain: "format", action: "sql" },
  "tool:network:tcp-test": { domain: "network", action: "tcp_test" },
  "tool:network:http-test": { domain: "network", action: "http_test" },
  "tool:env:detect": { domain: "env", action: "detect" },
  "tool:port:usage": { domain: "port", action: "usage" },
  "tool:file:split": { domain: "file", action: "split" },
  "tool:file:merge": { domain: "file", action: "merge" },
  "tool:image:convert": { domain: "image", action: "convert" },
  "tool:hosts:save": { domain: "hosts", action: "save" },
  "tool:hosts:list": { domain: "hosts", action: "list" },
  "tool:hosts:delete": { domain: "hosts", action: "delete" },
  "tool:hosts:activate": { domain: "hosts", action: "activate" },
  "tool:manuals:list": { domain: "manuals", action: "list" }
};

export async function invokeToolByChannel(
  channel: string,
  payload: Record<string, unknown>
): Promise<unknown> {
  const mapping = CHANNEL_MAP[channel];
  if (!mapping) {
    throw new Error(`Unsupported channel: ${channel}`);
  }

  const request: ToolRequest = {
    request_id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    domain: mapping.domain,
    action: mapping.action,
    payload
  };

  try {
    if (typeof invoke !== "function") {
      throw new Error("IPC bridge unavailable");
    }
    const response = await invoke<ToolResponse>("tool_execute", { request });
    if (!response.ok) {
      throw new Error(response.error?.message ?? "调用失败");
    }
    return response.data;
  } catch (error) {
    const message = (error as Error).message ?? "";
    if (
      message.includes("unknown IPC") ||
      message.includes("failed to fetch") ||
      message.includes("IPC bridge unavailable") ||
      message.includes("reading 'invoke'")
    ) {
      throw new Error("IPC bridge 未加载，请在 Tauri 环境运行。请使用 `pnpm dev` 或 `pnpm --filter @lazycat/desktop dev` 启动。");
    }
    throw error;
  }
}
