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

export async function registerNamedHotkey(name: string, shortcut: string): Promise<void> {
  await invoke("register_named_hotkey", { name, shortcut });
}

export async function unregisterNamedHotkey(name: string): Promise<void> {
  await invoke("unregister_named_hotkey", { name });
}

export async function pauseAllShortcuts(): Promise<void> {
  await invoke("pause_all_shortcuts");
}

export async function resumeAllShortcuts(): Promise<void> {
  await invoke("resume_all_shortcuts");
}

export async function suppressClipboardCapture(content: string): Promise<void> {
  await invoke("suppress_clipboard_capture", { content });
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
  "tool:convert:java-bean-to-json": { domain: "convert", action: "java_bean_to_json" },
  "tool:convert:json-to-js-object": { domain: "convert", action: "json_to_js_object" },
  "tool:convert:java-bean-to-js-object": { domain: "convert", action: "java_bean_to_js_object" },
  "tool:convert:config-convert": { domain: "convert", action: "config_convert" },
  "tool:convert:yaml-validate": { domain: "convert", action: "yaml_validate" },
  "tool:convert:yaml-format": { domain: "convert", action: "yaml_format" },
  "tool:convert:sql-to-entity": { domain: "convert", action: "sql_to_entity" },
  "tool:text:process": { domain: "text", action: "process" },
  "tool:text:presets": { domain: "text", action: "presets" },
  "tool:text:naming-convert": { domain: "text", action: "naming_convert" },
  "tool:time:timestamp-to-date": { domain: "time", action: "timestamp_to_date" },
  "tool:time:date-to-timestamp": { domain: "time", action: "date_to_timestamp" },
  "tool:time:date-diff": { domain: "time", action: "date_diff" },
  "tool:time:date-add": { domain: "time", action: "date_add" },
  "tool:gen:uuid": { domain: "gen", action: "uuid" },
  "tool:gen:uuid-simple": { domain: "gen", action: "uuid_simple" },
  "tool:gen:guid": { domain: "gen", action: "guid" },
  "tool:gen:snowflake": { domain: "gen", action: "snowflake" },
  "tool:gen:password": { domain: "gen", action: "password" },
  "tool:gen:password-strength": { domain: "gen", action: "password_strength" },
  "tool:regex:test": { domain: "regex", action: "test" },
  "tool:regex:generate": { domain: "regex", action: "generate" },
  "tool:regex:templates": { domain: "regex", action: "templates" },
  "tool:regex:replace": { domain: "regex", action: "replace" },
  "tool:cron:generate": { domain: "cron", action: "generate" },
  "tool:cron:preview": { domain: "cron", action: "preview" },
  "tool:cron:preview-v2": { domain: "cron", action: "preview_v2" },
  "tool:cron:parse": { domain: "cron", action: "parse" },
  "tool:cron:normalize": { domain: "cron", action: "normalize" },
  "tool:cron:describe": { domain: "cron", action: "describe" },
  "tool:crypto:rsa-encrypt": { domain: "crypto", action: "rsa_encrypt" },
  "tool:crypto:rsa-decrypt": { domain: "crypto", action: "rsa_decrypt" },
  "tool:crypto:aes-encrypt": { domain: "crypto", action: "aes_encrypt" },
  "tool:crypto:aes-decrypt": { domain: "crypto", action: "aes_decrypt" },
  "tool:crypto:des-encrypt": { domain: "crypto", action: "des_encrypt" },
  "tool:crypto:des-decrypt": { domain: "crypto", action: "des_decrypt" },
  "tool:crypto:bcrypt-hash": { domain: "crypto", action: "bcrypt_hash" },
  "tool:crypto:bcrypt-verify": { domain: "crypto", action: "bcrypt_verify" },
  "tool:format:json": { domain: "format", action: "json" },
  "tool:format:xml": { domain: "format", action: "xml" },
  "tool:format:html": { domain: "format", action: "html" },
  "tool:format:java": { domain: "format", action: "java" },
  "tool:format:sql": { domain: "format", action: "sql" },
  "tool:network:tcp-test": { domain: "network", action: "tcp_test" },
  "tool:network:udp-test": { domain: "network", action: "udp_test" },
  "tool:network:ping-test": { domain: "network", action: "ping_test" },
  "tool:network:http-test": { domain: "network", action: "http_test" },
  "tool:network:http-status-list": { domain: "network", action: "http_status_list" },
  "tool:network:http-status-lookup": { domain: "network", action: "http_status_lookup" },
  "tool:network:chmod-calc": { domain: "network", action: "chmod_calc" },
  "tool:dns:resolve": { domain: "dns", action: "resolve" },
  "tool:dns:system-dns": { domain: "dns", action: "system_dns" },
  "tool:dns:compare": { domain: "dns", action: "compare" },
  "tool:env:detect": { domain: "env", action: "detect" },
  "tool:port:usage": { domain: "port", action: "usage" },
  "tool:port:process-detail": { domain: "port", action: "process_detail" },
  "tool:port:kill": { domain: "port", action: "kill" },
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
  "tool:settings:enable-autostart": { domain: "settings", action: "enable_autostart" },
  "tool:settings:disable-autostart": { domain: "settings", action: "disable_autostart" },
  "tool:settings:is-autostart-enabled": { domain: "settings", action: "is_autostart_enabled" },
  "tool:jwt:decode": { domain: "jwt", action: "decode" },
  "tool:hotkey:check": { domain: "hotkey", action: "check" },
  "tool:hotkey:scan": { domain: "hotkey", action: "scan" },
  "tool:hotkey:mappings": { domain: "hotkey", action: "mappings" },
  "tool:hotkey:detect-owner": { domain: "hotkey", action: "detect_owner" },
  "tool:schema:validate": { domain: "schema", action: "validate" },
  "tool:schema:generate-example": { domain: "schema", action: "generate_example" },
  "tool:mybatis:render": { domain: "mybatis", action: "render" },
  "tool:mybatis:lint": { domain: "mybatis", action: "lint" },
  "tool:mybatis:extract-params": { domain: "mybatis", action: "extract_params" },
  "tool:nginx:generate": { domain: "nginx", action: "generate" },
  "tool:nginx:lint": { domain: "nginx", action: "lint" },
  "tool:snippets:v2:list": { domain: "snippets", action: "v2_list" },
  "tool:snippets:v2:get": { domain: "snippets", action: "v2_get" },
  "tool:snippets:v2:create": { domain: "snippets", action: "v2_create" },
  "tool:snippets:v2:update": { domain: "snippets", action: "v2_update" },
  "tool:snippets:v2:delete": { domain: "snippets", action: "v2_delete" },
  "tool:snippets:v2:search": { domain: "snippets", action: "v2_search" },
  "tool:snippets:v2:mark-used": { domain: "snippets", action: "v2_mark_used" },
  "tool:snippets:v2:tag-stats": { domain: "snippets", action: "v2_tag_stats" },
  "tool:snippets:v2:folder-list": { domain: "snippets", action: "v2_folder_list" },
  "tool:snippets:v2:folder-create": { domain: "snippets", action: "v2_folder_create" },
  "tool:snippets:v2:folder-update": { domain: "snippets", action: "v2_folder_update" },
  "tool:snippets:v2:folder-delete": { domain: "snippets", action: "v2_folder_delete" },
  "tool:pdf:info":  { domain: "pdf", action: "info" },
  "tool:pdf:split": { domain: "pdf", action: "split" },
  "tool:pdf:merge": { domain: "pdf", action: "merge" },
  "tool:vault:status":          { domain: "vault", action: "status" },
  "tool:vault:setup":           { domain: "vault", action: "setup" },
  "tool:vault:unlock":          { domain: "vault", action: "unlock" },
  "tool:vault:touch":           { domain: "vault", action: "touch" },
  "tool:vault:lock":            { domain: "vault", action: "lock" },
  "tool:vault:change-password": { domain: "vault", action: "change_password" },
  "tool:vault:list":            { domain: "vault", action: "list" },
  "tool:vault:meta-list":       { domain: "vault", action: "meta_list" },
  "tool:vault:get":             { domain: "vault", action: "get" },
  "tool:vault:reveal-one":      { domain: "vault", action: "reveal_one" },
  "tool:vault:create":          { domain: "vault", action: "create" },
  "tool:vault:update":          { domain: "vault", action: "update" },
  "tool:vault:delete":          { domain: "vault", action: "delete" },
  "tool:vault:open-url":        { domain: "vault", action: "open_url" },
  "tool:vault:tag-stats":       { domain: "vault", action: "tag_stats" },
  "tool:vault:rename-tag":      { domain: "vault", action: "rename_tag" },
  "tool:vault:delete-tag":      { domain: "vault", action: "delete_tag" },
  "tool:vault:record-usage":    { domain: "vault", action: "record_usage" },
  "tool:launcher:scan":          { domain: "launcher", action: "scan" },
  "tool:launcher:list":          { domain: "launcher", action: "list" },
  "tool:launcher:add":           { domain: "launcher", action: "add" },
  "tool:launcher:add-manual":    { domain: "launcher", action: "add_manual" },
  "tool:launcher:update":        { domain: "launcher", action: "update" },
  "tool:launcher:remove":        { domain: "launcher", action: "remove" },
  "tool:launcher:reorder":       { domain: "launcher", action: "reorder" },
  "tool:launcher:launch":        { domain: "launcher", action: "launch" },
  "tool:launcher:open-folder":   { domain: "launcher", action: "open_folder" },
  "tool:launcher:list-groups":   { domain: "launcher", action: "list_groups" },
  "tool:launcher:create-group":  { domain: "launcher", action: "create_group" },
  "tool:launcher:rename-group":  { domain: "launcher", action: "rename_group" },
  "tool:launcher:delete-group":  { domain: "launcher", action: "delete_group" },
  "tool:todo:type-list": { domain: "todo", action: "type_list" },
  "tool:todo:type-upsert": { domain: "todo", action: "type_upsert" },
  "tool:todo:type-delete": { domain: "todo", action: "type_delete" },
  "tool:todo:assignee-list": { domain: "todo", action: "assignee_list" },
  "tool:todo:assignee-upsert": { domain: "todo", action: "assignee_upsert" },
  "tool:todo:assignee-delete": { domain: "todo", action: "assignee_delete" },
  "tool:todo:item-list": { domain: "todo", action: "item_list" },
  "tool:todo:item-create": { domain: "todo", action: "item_create" },
  "tool:todo:item-update": { domain: "todo", action: "item_update" },
  "tool:todo:item-upsert": { domain: "todo", action: "item_upsert" },
  "tool:todo:item-change-status": { domain: "todo", action: "item_change_status" },
  "tool:todo:item-snooze": { domain: "todo", action: "item_snooze" },
  "tool:todo:item-toggle-pin": { domain: "todo", action: "item_toggle_pin" },
  "tool:todo:item-toggle-active": { domain: "todo", action: "item_toggle_active" },
  "tool:todo:item-delete": { domain: "todo", action: "item_delete" },
  "tool:todo:reminder-list-unread": { domain: "todo", action: "reminder_list_unread" },
  "tool:todo:reminder-mark-read": { domain: "todo", action: "reminder_mark_read" },
  "tool:todo:open-link":          { domain: "todo", action: "open_link" },
  "tool:maven:locate": { domain: "maven", action: "locate" },
  "tool:maven:open-path": { domain: "maven", action: "open_path" },
  "tool:inbox:list": { domain: "inbox", action: "list" },
  "tool:inbox:get": { domain: "inbox", action: "get" },
  "tool:inbox:search": { domain: "inbox", action: "search" },
  "tool:inbox:promote": { domain: "inbox", action: "promote" },
  "tool:inbox:update-meta": { domain: "inbox", action: "update_meta" },
  "tool:inbox:archive": { domain: "inbox", action: "archive" },
  "tool:inbox:delete": { domain: "inbox", action: "delete" },
  "tool:inbox:cleanup": { domain: "inbox", action: "cleanup" },
  "tool:inbox:capture-status": { domain: "inbox", action: "capture_status" },
  "tool:inbox:capture-pause": { domain: "inbox", action: "capture_pause" },
  "tool:inbox:open-path": { domain: "inbox", action: "open_path" },
  "tool:inbox:copy-image": { domain: "inbox", action: "copy_image" },
  "tool:pm:project-list": { domain: "pm", action: "project_list" },
  "tool:pm:project-create": { domain: "pm", action: "project_create" },
  "tool:pm:project-update": { domain: "pm", action: "project_update" },
  "tool:pm:project-archive": { domain: "pm", action: "project_archive" },
  "tool:pm:project-restore": { domain: "pm", action: "project_restore" },
  "tool:pm:project-delete": { domain: "pm", action: "project_delete" },
  "tool:pm:item-counts": { domain: "pm", action: "item_counts" },
  "tool:pm:item-list": { domain: "pm", action: "item_list" },
  "tool:pm:item-create": { domain: "pm", action: "item_create" },
  "tool:pm:item-update": { domain: "pm", action: "item_update" },
  "tool:pm:item-change-status": { domain: "pm", action: "item_change_status" },
  "tool:pm:item-reorder": { domain: "pm", action: "item_reorder" },
  "tool:pm:item-toggle-pin": { domain: "pm", action: "item_toggle_pin" },
  "tool:pm:item-batch-update": { domain: "pm", action: "item_batch_update" },
  "tool:pm:item-delete": { domain: "pm", action: "item_delete" },
  "tool:pm:item-move-project": { domain: "pm", action: "item_move_project" },
  "tool:pm:tag-list": { domain: "pm", action: "tag_list" },
  "tool:pm:weekly-work": { domain: "pm", action: "weekly_work" },
  "tool:pm:siyuan-test": { domain: "pm", action: "siyuan_test" },
  "tool:pm:siyuan-directory": { domain: "pm", action: "siyuan_directory" },
  "tool:pm:siyuan-search-pages": { domain: "pm", action: "siyuan_search_pages" },
  "tool:pm:siyuan-create-page": { domain: "pm", action: "siyuan_create_page" },
  "tool:pm:siyuan-open-page": { domain: "pm", action: "siyuan_open_page" },
  "tool:pm:open-link": { domain: "pm", action: "open_link" },
  "tool:pm:siyuan-check-running": { domain: "pm", action: "siyuan_check_running" },
  "tool:pm:siyuan-launch": { domain: "pm", action: "siyuan_launch" },
  "tool:pm:item-todo-list": { domain: "pm", action: "item_todo_list" },
  "tool:pm:item-todo-link": { domain: "pm", action: "item_todo_link" },
  "tool:pm:item-todo-unlink": { domain: "pm", action: "item_todo_unlink" },
  "tool:pm:item-todo-create": { domain: "pm", action: "item_todo_create" },
  "tool:pm:item-todo-candidates": { domain: "pm", action: "item_todo_candidates" },
  "tool:pm:item-todo-candidates-by-project": { domain: "pm", action: "item_todo_candidates_by_project" },
  "tool:pm:item-today-list": { domain: "pm", action: "item_today_list" },
  "tool:pm:item-today-counts": { domain: "pm", action: "item_today_counts" },
  "tool:pm:item-calendar-range": { domain: "pm", action: "item_calendar_range" },
  "tool:pm:item-matrix-bucket": { domain: "pm", action: "item_matrix_bucket" },
  "tool:pm:item-import-preview": { domain: "pm", action: "item_import_preview" },
  "tool:pm:item-import": { domain: "pm", action: "item_import" },
  "tool:todo:pm-candidates": { domain: "todo", action: "pm_candidates" },
  "tool:todo:item-set-pm-link": { domain: "todo", action: "item_set_pm_link" },
  "tool:attachments:save":             { domain: "attachments", action: "save" },
  "tool:attachments:save-from-path":   { domain: "attachments", action: "save_from_path" },
  "tool:attachments:list":             { domain: "attachments", action: "list" },
  "tool:attachments:remove":           { domain: "attachments", action: "remove" },
  "tool:attachments:rebind":           { domain: "attachments", action: "rebind" },
  "tool:attachments:cleanup-orphans":  { domain: "attachments", action: "cleanup_orphans" },
  "tool:attachments:delete-by-owner":  { domain: "attachments", action: "delete_by_owner" },
  "tool:system:get-paths":             { domain: "system", action: "get_paths" },
  "tool:system:open-external":         { domain: "system", action: "open_external" },
  "tool:system:read-clipboard-files":  { domain: "system", action: "read_clipboard_files" },
  "tool:system:open-local-path":       { domain: "system", action: "open_local_path" },
  "tool:system:reveal-in-folder":      { domain: "system", action: "reveal_in_folder" },
  "tool:system:check-paths-exist":     { domain: "system", action: "check_paths_exist" },
  "tool:widget:dashboard-data":     { domain: "widget", action: "dashboard_data" },
  "tool:widget:apply":              { domain: "widget", action: "apply" },
  "tool:widget:pause":              { domain: "widget", action: "pause" },
  "tool:widget:resume":             { domain: "widget", action: "resume" },
  "tool:widget:status":             { domain: "widget", action: "status" },
  "tool:widget:enable":             { domain: "widget", action: "enable" },
  "tool:widget:disable":            { domain: "widget", action: "disable" },
  "tool:widget:get-config":         { domain: "widget", action: "get_config" },
  "tool:widget:set-config":         { domain: "widget", action: "set_config" },
  "tool:widget:set-privacy-mask":   { domain: "widget", action: "set_privacy_mask" },
  "tool:widget:diagnostics":       { domain: "widget", action: "diagnostics" },
  "tool:widget:reposition":        { domain: "widget", action: "reposition" }
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
