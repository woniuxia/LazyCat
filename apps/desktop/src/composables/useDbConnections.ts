import { ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type { DbConnection, DbConnectionDraft } from "../types/db";

/**
 * 数据库工作台连接状态管理：连接列表 CRUD 与"已打开连接"的会话态
 * （服务器版本、库列表）。打开态只存在于前端会话内，面板关闭即失。
 */

export interface OpenedConnection {
  connectionId: string;
  serverVersion: string;
  databases: string[];
  /** 当前选中的库 */
  activeDatabase: string;
}

export function useDbConnections() {
  const connections = ref<DbConnection[]>([]);
  const loading = ref(false);
  /** connectionId -> 打开态 */
  const opened = ref<Map<string, OpenedConnection>>(new Map());

  async function refresh(): Promise<void> {
    loading.value = true;
    try {
      const data = (await invokeToolByChannel("tool:db:connection-list", {})) as {
        connections: DbConnection[];
      };
      connections.value = data.connections;
    } finally {
      loading.value = false;
    }
  }

  async function save(draft: DbConnectionDraft): Promise<string> {
    const payload: Record<string, unknown> = {
      id: draft.id,
      name: draft.name,
      engine: draft.engine,
      host: draft.host,
      port: draft.port,
      username: draft.username,
      defaultDatabase: draft.defaultDatabase,
      envTag: draft.envTag,
      readOnly: draft.readOnly,
      groupName: draft.groupName,
      options: draft.options,
    };
    // 密码占位符语义：undefined 表示未改动，不发送该字段
    if (draft.password !== undefined) {
      payload.password = draft.password;
    }
    const data = (await invokeToolByChannel("tool:db:connection-save", payload)) as { id: string };
    await refresh();
    return data.id;
  }

  async function remove(connectionId: string): Promise<void> {
    await invokeToolByChannel("tool:db:connection-delete", { connectionId });
    opened.value.delete(connectionId);
    opened.value = new Map(opened.value);
    await refresh();
  }

  async function open(connectionId: string): Promise<OpenedConnection> {
    const data = (await invokeToolByChannel("tool:db:connection-open", { connectionId })) as {
      serverVersion: string;
      databases: string[];
      defaultDatabase: string | null;
    };
    const state: OpenedConnection = {
      connectionId,
      serverVersion: data.serverVersion,
      databases: data.databases,
      activeDatabase:
        data.defaultDatabase && data.databases.includes(data.defaultDatabase)
          ? data.defaultDatabase
          : data.databases[0] ?? "",
    };
    opened.value.set(connectionId, state);
    opened.value = new Map(opened.value);
    return state;
  }

  async function close(connectionId: string): Promise<void> {
    await invokeToolByChannel("tool:db:connection-close", { connectionId });
    opened.value.delete(connectionId);
    opened.value = new Map(opened.value);
  }

  function setActiveDatabase(connectionId: string, database: string): void {
    const state = opened.value.get(connectionId);
    if (state) {
      state.activeDatabase = database;
      opened.value = new Map(opened.value);
    }
  }

  return {
    connections,
    loading,
    opened,
    refresh,
    save,
    remove,
    open,
    close,
    setActiveDatabase,
  };
}
