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
  "tool:encode:base64-url-encode": { domain: "encode", action: "base64_url_encode" },
  "tool:encode:base64-url-decode": { domain: "encode", action: "base64_url_decode" },
  "tool:encode:url-encode": { domain: "encode", action: "url_encode" },
  "tool:encode:url-decode": { domain: "encode", action: "url_decode" },
  "tool:encode:md5": { domain: "encode", action: "md5" },
  "tool:encode:sha1": { domain: "encode", action: "sha1" },
  "tool:encode:sha256": { domain: "encode", action: "sha256" },
  "tool:encode:sha512": { domain: "encode", action: "sha512" },
  "tool:encode:hmac-sha256": { domain: "encode", action: "hmac_sha256" },
  "tool:encode:qr": { domain: "encode", action: "qr_generate" },
  "tool:convert:json-to-xml": { domain: "convert", action: "json_to_xml" },
  "tool:convert:xml-to-json": { domain: "convert", action: "xml_to_json" },
  "tool:convert:json-to-yaml": { domain: "convert", action: "json_to_yaml" },
  "tool:convert:csv-to-json": { domain: "convert", action: "csv_to_json" },
  "tool:convert:csv-read-file": { domain: "convert", action: "csv_read_file" },
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
  "tool:regex:replace": { domain: "regex", action: "replace" },
  "tool:cron:generate": { domain: "cron", action: "generate" },
  "tool:cron:preview": { domain: "cron", action: "preview" },
  "tool:cron:parse": { domain: "cron", action: "parse" },
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
  "tool:dns:resolve": { domain: "dns", action: "resolve" },
  "tool:dns:system-dns": { domain: "dns", action: "system_dns" },
  "tool:env:detect": { domain: "env", action: "detect" },
  "tool:port:usage": { domain: "port", action: "usage" },
  "tool:file:split": { domain: "file", action: "split" },
  "tool:file:merge": { domain: "file", action: "merge" },
  "tool:file:write-text": { domain: "file", action: "write_text" },
  "tool:image:convert": { domain: "image", action: "convert" },
  "tool:image:info": { domain: "image", action: "info" },
  "tool:hosts:save": { domain: "hosts", action: "save" },
  "tool:hosts:list": { domain: "hosts", action: "list" },
  "tool:hosts:delete": { domain: "hosts", action: "delete" },
  "tool:hosts:activate": { domain: "hosts", action: "activate" },
  "tool:hosts:reorder": { domain: "hosts", action: "reorder" },
  "tool:hosts:read-system": { domain: "hosts", action: "read_system" },
  "tool:hosts:admin-check": { domain: "hosts", action: "admin_check" },
  "tool:hosts:backup-list": { domain: "hosts", action: "backup_list" },
  "tool:hosts:backup-restore": { domain: "hosts", action: "backup_restore" },
  "tool:manuals:list": { domain: "manuals", action: "list" },
  "tool:settings:get": { domain: "settings", action: "get" },
  "tool:settings:set": { domain: "settings", action: "set" },
  "tool:settings:get-all": { domain: "settings", action: "get_all" },
  "tool:settings:export": { domain: "settings", action: "export" },
  "tool:settings:import": { domain: "settings", action: "import" },
  "tool:settings:export-to-file": { domain: "settings", action: "export_to_file" },
  "tool:settings:import-from-file": { domain: "settings", action: "import_from_file" },
  "tool:settings:get-data-dir": { domain: "settings", action: "get_data_dir" },
  "tool:settings:set-data-dir": { domain: "settings", action: "set_data_dir" },
  "tool:settings:reset-data-dir": { domain: "settings", action: "reset_data_dir" },
  "tool:jwt:decode": { domain: "jwt", action: "decode" },
  "tool:hotkey:check": { domain: "hotkey", action: "check" },
  "tool:hotkey:scan": { domain: "hotkey", action: "scan" },
  "tool:hotkey:mappings": { domain: "hotkey", action: "mappings" },
  "tool:hotkey:detect-owner": { domain: "hotkey", action: "detect_owner" }
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
