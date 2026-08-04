<template>
  <el-dialog
    v-model="visible"
    title="加密与安全设置"
    width="min(520px, calc(100vw - 32px))"
    :close-on-click-modal="false"
    class="vault-security-dialog"
    @open="reloadSettings"
  >
    <div class="vault-security-settings">
      <div class="vault-security-setting">
        <div class="vault-security-setting-copy">
          <span class="vault-security-setting-label">敏感信息隐藏</span>
          <span class="vault-security-setting-desc"
            >无操作达到时长后恢复密码掩码，窗口失焦时立即隐藏</span
          >
        </div>
        <el-select
          v-model="settings.sensitiveHideMinutes"
          class="vault-security-minutes"
          :disabled="saving"
          aria-label="敏感信息隐藏时间"
          @change="saveSetting('sensitiveHideMinutes')"
        >
          <el-option
            v-for="minutes in VAULT_SENSITIVE_HIDE_MINUTES"
            :key="minutes"
            :label="`${minutes} 分钟`"
            :value="minutes"
          />
        </el-select>
      </div>

      <div class="vault-security-setting">
        <div class="vault-security-setting-copy">
          <span class="vault-security-setting-label">密码库无活动自动锁定</span>
          <span class="vault-security-setting-desc">没有操作密码库达到时长后清除解锁会话</span>
        </div>
        <div class="vault-security-rule">
          <el-switch
            v-model="settings.activityLockEnabled"
            :disabled="saving"
            aria-label="启用密码库无活动自动锁定"
            @change="saveSetting('activityLockEnabled')"
          />
          <el-select
            v-model="settings.activityLockMinutes"
            class="vault-security-minutes"
            :disabled="saving || !settings.activityLockEnabled"
            aria-label="密码库无活动自动锁定时间"
            @change="saveSetting('activityLockMinutes')"
          >
            <el-option
              v-for="minutes in VAULT_HARD_LOCK_MINUTES"
              :key="minutes"
              :label="`${minutes} 分钟`"
              :value="minutes"
            />
          </el-select>
        </div>
      </div>

      <div class="vault-security-setting">
        <div class="vault-security-setting-copy">
          <span class="vault-security-setting-label">电脑无操作自动锁定</span>
          <span class="vault-security-setting-desc">整台电脑没有键盘或鼠标输入达到时长后锁定</span>
        </div>
        <div class="vault-security-rule">
          <el-switch
            v-model="settings.systemIdleLockEnabled"
            :disabled="saving"
            aria-label="启用电脑无操作自动锁定"
            @change="saveSetting('systemIdleLockEnabled')"
          />
          <el-select
            v-model="settings.systemIdleLockMinutes"
            class="vault-security-minutes"
            :disabled="saving || !settings.systemIdleLockEnabled"
            aria-label="电脑无操作自动锁定时间"
            @change="saveSetting('systemIdleLockMinutes')"
          >
            <el-option
              v-for="minutes in VAULT_HARD_LOCK_MINUTES"
              :key="minutes"
              :label="`${minutes} 分钟`"
              :value="minutes"
            />
          </el-select>
        </div>
      </div>

      <p class="vault-security-summary">当前策略：{{ lockSummary }}</p>
    </div>
    <template #footer>
      <el-button @click="visible = false">完成</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { getVaultLockSettings, setVaultLockSettingAndWait } from "../composables/useSettings";
import {
  summarizeVaultHardLockRules,
  VAULT_HARD_LOCK_MINUTES,
  VAULT_SENSITIVE_HIDE_MINUTES,
  type VaultLockSettings,
} from "../utils/vaultLock";

const visible = ref(false);
const saving = ref(false);
const settings = reactive(getVaultLockSettings());
const lockSummary = computed(() => summarizeVaultHardLockRules(settings));

function reloadSettings() {
  Object.assign(settings, getVaultLockSettings());
}

function show() {
  reloadSettings();
  visible.value = true;
}

async function saveSetting<K extends keyof VaultLockSettings>(name: K) {
  saving.value = true;
  try {
    await setVaultLockSettingAndWait(name, settings[name]);
    reloadSettings();
    ElMessage.success("安全设置已更新");
  } catch (error) {
    reloadSettings();
    ElMessage.error(`设置失败：${(error as Error).message}`);
    return;
  } finally {
    saving.value = false;
  }
  try {
    await invokeToolByChannel("tool:vault:status", {});
  } catch {
    // 设置已保存，密码库面板仍会通过订阅和状态轮询同步。
  }
}

defineExpose({ show });
</script>

<style scoped>
.vault-security-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.vault-security-setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  min-height: 64px;
  padding: 14px 2px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.vault-security-setting:last-of-type {
  border-bottom: none;
}

.vault-security-setting-copy {
  min-width: 0;
}

.vault-security-setting-label,
.vault-security-setting-desc {
  display: block;
}

.vault-security-setting-label {
  margin-bottom: 4px;
  font-size: 14px;
  font-weight: 500;
  color: var(--lc-text);
}

.vault-security-setting-desc {
  font-size: 12px;
  line-height: 1.5;
  color: var(--lc-text-muted);
}

.vault-security-rule {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.vault-security-minutes {
  width: 120px;
  flex-shrink: 0;
}

.vault-security-summary {
  margin: 2px 2px 0;
  font-size: 12px;
  line-height: 1.6;
  text-align: right;
  color: var(--lc-text-muted);
}

@media (max-width: 560px) {
  .vault-security-setting {
    align-items: flex-start;
    flex-direction: column;
    gap: 10px;
  }

  .vault-security-rule {
    width: 100%;
    justify-content: space-between;
  }

  .vault-security-setting > .vault-security-minutes {
    align-self: flex-end;
  }
}
</style>

<style>
.vault-security-dialog .el-dialog__header {
  margin-right: 0;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--lc-border);
}

.vault-security-dialog .el-dialog__title {
  font-family: var(--lc-font-display);
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
}
</style>
