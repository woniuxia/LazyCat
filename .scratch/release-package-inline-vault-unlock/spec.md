Type: task
Labels: ready-for-agent

# 上线包 Vault 原地解锁

## Problem Statement

上线包使用密码认证上传服务器时，只保存密码库服务器凭据的引用，并在运行时从已解锁的 Vault 读取密码。当前 Vault 一旦锁定，首次上传、上传失败重试或上传后命令重试都会被阻断，用户必须离开当前确认上下文、跳转到密码管理工具完成解锁，再返回上线包重新发起操作。这个流程丢失操作连续性，也会迫使用户重复生产确认、主机探测或上传确认。

## Solution

在上线包现有确认弹窗中提供 Vault 原地解锁。流程在生产检查和 SSH 主机指纹确认完成后检查 Vault 状态；若 Vault 已锁定，则在当前弹窗展示绑定凭据摘要和主密码输入。用户通过“解锁并继续”按钮或 Enter 显式校验主密码，成功后建立正常的应用级 Vault 解锁会话，并自动恢复刚才被阻断的认证预检与交付流程。

首次上传、上传失败重试和上传后命令重试使用相同的解锁控件、错误语义及敏感数据清理规则。主密码错误时留在原弹窗内反馈，不跳转页面；无法由解锁解决的凭据配置错误继续显示对应错误和密码管理入口。

## User Stories

1. As an operator, I want to unlock Vault inside the release-package confirmation dialog, so that I do not lose my current deployment context.
2. As an operator, I want initial server uploads to support inline unlock, so that a locked Vault does not force me to restart the packaging flow.
3. As an operator, I want failed-upload retries to support inline unlock, so that recovery does not require a detour through password management.
4. As an operator, I want post-upload command retries to support inline unlock, so that already uploaded server files are not confused with a new upload attempt.
5. As an operator, I want the same inline-unlock behavior in all three flows, so that I do not have to learn different credential interactions.
6. As an operator, I want to see which Vault credential is bound to the current release environment, so that I know which server identity will be used.
7. As an operator, I want the main-password field to appear only when Vault is locked, so that an unlocked session does not appear to require another password.
8. As an operator, I want the existing credential summary when Vault is already unlocked, so that the normal confirmation dialog remains concise.
9. As an operator, I want production checks to happen before credential use, so that invalid production state is rejected before I enter a secret.
10. As an operator, I want the SSH host fingerprint confirmed before Vault unlock and SSH authentication, so that credentials are only used after the server identity is trusted.
11. As an operator, I want to submit the main password explicitly with a button or Enter, so that incomplete input does not trigger expensive or accidental validation attempts.
12. As an operator, I want a successful unlock to resume the exact interrupted action automatically, so that I do not need to click the original confirmation again.
13. As an operator, I want a wrong main password reported next to the input, so that I can immediately understand and correct the failure.
14. As an operator, I want an incorrect password cleared and the field focused again, so that the secret does not remain visible or require manual selection.
15. As an operator, I want production confirmation and trusted-host context preserved after a wrong password, so that unrelated confirmations are not repeated.
16. As an operator, I want a still-valid host probe preserved during unlock retries, so that password correction does not repeat network work.
17. As an operator, I want an expired probe token rebuilt automatically, so that token lifetime does not leave the dialog in an unrecoverable state.
18. As an operator, I want cancellation or dialog closure to clear the entered main password, so that transient secrets do not survive the current interaction.
19. As an operator, I want the main password cleared immediately after every unlock attempt, so that packaging, upload, logs and notifications never retain it.
20. As an operator, I want an inline unlock to create the same Vault session as password management, so that the existing automatic-lock policy remains the only session policy.
21. As an operator, I want a preflight-approved upload to continue if Vault locks afterward, so that a short-lived single-use authentication token remains usable for its intended action.
22. As an operator, I want a newly locked Vault detected again before credential use, so that a stale front-end status cannot bypass the session boundary.
23. As an operator, I want missing, wrong-category or incomplete Vault bindings reported as configuration errors, so that I am not asked for a password that cannot fix them.
24. As an operator, I want the password-management shortcut retained for configuration errors, so that invalid credentials can still be repaired deliberately.
25. As an operator, I do not want the previous “open password management to unlock” interruption after a lock error, so that the release-package flow remains local.
26. As an operator, I want only one unlock or start continuation active at a time, so that repeated clicks cannot create duplicate preflights or deployments.
27. As an operator, I want Vault status and unlock failures exposed explicitly, so that infrastructure errors are not mistaken for a wrong password or successful unlock.
28. As an operator, I want private-key authentication to keep its current behavior, so that this password-bound Vault optimization does not change unrelated SSH authentication.

## Implementation Decisions

- Cover initial server upload, failed-upload retry and post-upload command retry. Local archive delivery and private-key authentication do not enter the Vault inline-unlock flow.
- Use one reusable inline-unlock control and one error mapping across the three confirmation contexts. Each context supplies its bound credential label and continuation action.
- Keep the current application-level Vault session as the only source of unlock truth. A successful inline unlock invokes the existing Vault unlock behavior and remains subject to existing activity and system-idle locking rules.
- Do not introduce release-package-only sessions, one-time main-password validation, direct server-password input or a second Vault state model.
- Separate SSH endpoint metadata loading from secret access. Password-bound host probing and command-retry preparation may read the bound entry's non-sensitive address, port and account while Vault is locked; they must not decrypt or obtain the server password.
- Preserve the security sequence: production validation and confirmation, host probe, host-fingerprint trust, Vault state check and inline unlock when required, SSH authentication preflight, remote overwrite confirmation, then start or retry.
- Require an unlocked Vault only at the stage that reads the encrypted server password for SSH authentication preflight. Recheck at this boundary so an automatic lock between earlier status inspection and credential use is handled.
- If the credential-use boundary returns a lock error after an earlier unlocked status, transition back to the same inline-unlock state and retain the pending action instead of navigating away or failing permanently.
- Submit the main password only on an explicit button click or Enter. Do not validate automatically while typing.
- On successful unlock, automatically continue the single pending operation. Guard unlock and continuation against duplicate clicks and overlapping requests.
- Wrong-password errors are rendered inline, clear the field and return focus. Other Vault or IPC errors remain explicit and distinguishable from wrong-password feedback.
- Preserve completed production confirmation, host trust and a still-valid host probe after unlock failure. If a short-lived probe has expired, recreate the probe and continue through the normal trust classification without retaining the main password.
- Clear the main-password value after every resolved or rejected unlock call, on cancellation, on dialog reset, on closure and on component disposal. Never put it in project configuration, persistent storage, logs, notifications, retry descriptions, probe tokens or preflight tokens.
- After SSH authentication preflight issues its existing short-lived single-use token, the current start or retry may consume that token even if the global Vault session locks. Do not request a redundant second unlock.
- Only a lock state presents the inline-unlock control. Vault-not-initialized, missing entry, invalid category and incomplete credential remain configuration errors and retain the existing password-management navigation entry.
- Remove the lock-error behavior that asks users to navigate to password management. Keep explicit mappings for the remaining Vault integration errors.
- Preserve existing token discard, cancellation and sensitive-start cleanup behavior. Unlock failures must not manufacture success, discard unrelated valid state or start a runtime task.
- No database or persisted project schema change is required. The release-package configuration continues storing only the Vault entry ID.
- Use the canonical glossary terms “上线包原地解锁” and “Vault 解锁会话” in relevant documentation and user-facing concepts.

## Testing Decisions

- Tests should assert observable dialog state, user actions, IPC contracts, security ordering and resulting continuation. They should not assert private helper names, source-code substrings or internal component structure.
- At the mounted Vue release-package panel seam, mock the existing invocation bridge and cover all three password-bound flows: locked-state rendering, bound credential summary, explicit unlock by button and Enter, wrong-password feedback, input clearing and refocus, successful automatic continuation, duplicate-submit prevention, cancellation cleanup and non-lock configuration errors.
- At the same panel seam, assert the externally visible call order: production confirmation where applicable, host probe and trust, Vault status/unlock, authentication preflight and start. Assert that private-key authentication does not render or call the Vault unlock flow.
- At the same panel seam, cover a Vault session that locks between initial status inspection and authentication preflight. The panel must return to inline unlock and resume without duplicating the start.
- At the Rust release-package action seam, verify that a password-bound host probe and command-retry preparation can resolve non-sensitive endpoint metadata while Vault is locked and do not read the server password.
- At the Rust action seam, verify that password authentication preflight still rejects a locked Vault, invalid binding, wrong category and incomplete secret data with distinguishable errors.
- At the Rust action seam, verify that a successfully issued single-use preflight token remains consumable after the Vault session locks, while preserving its existing environment, endpoint, target, expiry and one-time-consumption binding.
- Extend the existing upload-preflight and command-retry composable tests only where request cancellation, stale-response suppression, token expiry recovery or sensitive-state clearing cannot be covered reliably through the mounted panel.
- Reuse the repository's existing mounted component harness, bridge mocks, composable Vitest conventions and Rust release-package tests. Do not introduce a new end-to-end framework for this feature.
- Minimum automated verification is the targeted release-package Vue tests, targeted Rust release-package tests, workspace type checking, desktop web build and `git diff --check`.
- Because this changes a confirmation dialog, implementation acceptance should also inspect the light theme, wrong-password state, loading state, narrow-window layout, keyboard submission and content overflow in a running product UI. If product UI launch is not authorized during implementation, report visual runtime verification as outstanding rather than treating build success as visual acceptance.

## Out of Scope

- Entering or storing the SSH server password directly in the release-package page.
- Creating a release-package-only Vault session or single-entry decryption path.
- Changing the Vault encryption format, key derivation, automatic-lock policy or global session lifetime.
- Adding a new local attempt lockout or rate-limit policy solely to the release-package UI.
- Redesigning the password-management tool or its setup, entry-editing and security-settings experiences.
- Repairing missing, wrong-category or incomplete Vault entries from inside the release-package confirmation dialog.
- Changing private-key authentication, private-key passphrase behavior or SSH host-trust policy.
- Changing upload transaction, overwrite, rollback, post-upload command, health-check or notification semantics.
- Persisting confirmation-dialog state or main-password input across dialog closure, tab closure or application restart.
- Adding database migrations or changing the stored Vault binding from an entry reference to copied credential fields.
- Automatically starting the product UI, packaging installers or publishing a release as part of implementation.

## Further Notes

The project glossary defines “上线包原地解锁” as restoring server delivery within the current release-package confirmation context and “Vault 解锁会话” as the existing application-level session shared with password management. These terms deliberately exclude one-time entry authorization.

The existing release-package security model remains authoritative: host fingerprint trust precedes SSH authentication, server passwords are read only by the local Rust process, and authentication secrets live only in the short-lived single-use preflight chain. This feature changes where the user unlocks Vault, not where server passwords are stored or which process receives them.

The specification intentionally retains valid confirmation and probe state after a wrong main password. This retention does not include the password itself; the password is always cleared independently of the surrounding release context.
