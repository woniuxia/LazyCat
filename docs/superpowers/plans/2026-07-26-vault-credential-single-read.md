# Vault Credential Single-Read Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure Vault server credential metadata and encrypted password are read from one SQLite row snapshot.

**Architecture:** Keep the metadata-only helper unchanged for existing callers, but extract its field validation into a shared parser. Make `resolve_server_credential` query metadata and encrypted fields together, then parse and decrypt that captured row.

**Tech Stack:** Rust 2021, rusqlite, serde_json, zeroize, Cargo tests

---

### Task 1: Add the failing consistency regression

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/vault.rs:289-369`
- Test: `apps/desktop/src-tauri/src/tools/vault.rs:1672-1691`

- [ ] **Step 1: Add a deterministic test-only mutation hook**

Add a `#[cfg(test)]` thread-local mutation request and call it immediately after `server_credential_metadata` reads the metadata row:

```rust
#[cfg(test)]
std::thread_local! {
    static SERVER_CREDENTIAL_METADATA_MUTATION:
        std::cell::Cell<Option<(i64, i64)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
struct ServerCredentialMetadataMutationGuard;

#[cfg(test)]
impl Drop for ServerCredentialMetadataMutationGuard {
    fn drop(&mut self) {
        SERVER_CREDENTIAL_METADATA_MUTATION.with(|mutation| mutation.set(None));
    }
}
```

The mutation helper updates `category`, `plain_fields`, `iv`, and `encrypted_blob` from a replacement entry in one SQL statement and returns an explicit setup error.

- [ ] **Step 2: Extend the existing resolver test**

In `resolved_server_credential_requires_session_and_keeps_password_out_of_metadata`, insert a replacement credential, arm the mutation, resolve the original entry, and assert the result remains entirely from the original version:

```rust
insert_test_server_entry(&conn, 2, "new-host", 2200, "new-user", "new-secret");
let consistent = {
    let _mutation_guard = arm_server_credential_metadata_mutation(1, 2);
    resolve_server_credential(&conn, 1).expect("resolve credential from one row")
};
assert_eq!(consistent.metadata.address, "10.0.0.8");
assert_eq!(consistent.metadata.port, 22);
assert_eq!(consistent.metadata.account, "deploy");
assert_eq!(&*consistent.password, "secret");
```

- [ ] **Step 3: Run the test and verify RED**

```powershell
$env:RUSTC_WRAPPER='sccache'; cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml tools::vault::tests::resolved_server_credential_requires_session_and_keeps_password_out_of_metadata -- --exact --nocapture
```

Expected: FAIL because the current two-query resolver returns original metadata with `new-secret`.

### Task 2: Read and parse one row

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/vault.rs:289-369`

- [ ] **Step 1: Extract metadata parsing**

Move category and `plain_fields` validation from `server_credential_metadata` into:

```rust
fn parse_server_credential_metadata(
    entry_id: i64,
    category: &str,
    plain_fields: Option<&str>,
) -> Result<VaultServerCredentialMetadata, String>
```

Preserve current address/account trimming, explicit port validation, legacy port default `22`, and existing error codes.

- [ ] **Step 2: Query the complete row once in the resolver**

Replace the metadata call plus second query with one query:

```sql
SELECT category, plain_fields, iv, encrypted_blob
FROM vault_entries WHERE id = ?1
```

Parse metadata from the captured fields, then obtain the session key and decrypt the captured IV/blob. Preserve `Zeroizing`, key zeroization, and existing error mapping.

- [ ] **Step 3: Run the regression and verify GREEN**

Run the exact command from Task 1. Expected: 1 passed, 0 failed.

- [ ] **Step 4: Run Vault and release-package regression tests**

```powershell
$env:RUSTC_WRAPPER='sccache'; cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml tools::vault::tests -- --nocapture
$env:RUSTC_WRAPPER='sccache'; cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml release_package -- --nocapture
```

Expected: all local tests pass; SSH fixture tests may remain ignored.

### Task 3: Format, inspect, and commit

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/vault.rs`

- [ ] **Step 1: Format and verify formatting**

```powershell
cargo fmt --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo fmt --manifest-path apps\desktop\src-tauri\Cargo.toml -- --check
```

Expected: formatting check exits 0.

- [ ] **Step 2: Inspect the final diff**

```powershell
git diff -- apps/desktop/src-tauri/src/tools/vault.rs
git diff --check
```

Expected: only the test hook, regression assertions, shared parser, and single-row query changed; whitespace check exits 0.

- [ ] **Step 3: Commit the implementation**

```powershell
git add apps/desktop/src-tauri/src/tools/vault.rs docs/superpowers/plans/2026-07-26-vault-credential-single-read.md
git commit -m "fix(vault): 保持服务器凭据单次读取"
```
