# Vault 服务器凭据单次读取设计

## 状态

- 日期：2026-07-26
- 状态：已确认，待实施
- 范围：保证 Vault 内部解析服务器凭据时，元数据和密文来自同一次数据库读取

## 背景

`resolve_server_credential` 当前先调用 `server_credential_metadata` 读取条目类型、地址、端口和账号，再单独查询 IV 与密文。如果同一条目恰好在两次查询之间被更新，返回值可能组合不同版本的元数据和密码。

## 设计

- 保留 `server_credential_metadata` 供只需要非敏感元数据的调用方使用。
- 提取共享的服务器元数据解析函数，集中校验条目类型、地址、端口和账号。
- `resolve_server_credential` 使用一次 SQL 查询读取 `category`、`plain_fields`、`iv` 和 `encrypted_blob`，随后从该行解析元数据并解密密码。
- 继续使用现有 Vault 会话检查、`Zeroizing` 密码容器和错误码，不修改 IPC、数据库 schema 或前端行为。

## 测试

- 增加确定性回归测试：在旧实现的元数据读取和密文读取之间替换条目内容，证明旧实现会组合不同版本的数据。
- 单次查询实现后，回归测试应返回同一版本的地址、端口、账号和密码。
- 运行 Vault 定向测试、上线包相关 Rust 测试、格式检查和 `git diff --check`。

## 非目标

- 不处理上传端点解析与后续凭据解密之间的更大时间窗口。
- 不调整预检令牌、schema 迁移、Vue 组件或上传运行时。
- 不合并 `feat/vault-release-upload` 分支。
