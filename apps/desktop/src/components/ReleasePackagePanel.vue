<template>
  <section class="release-package-panel">
    <div class="release-package-workspace">
      <aside class="release-package-projects" aria-label="项目列表">
        <div class="projects-heading">
          <strong>项目配置</strong>
          <div class="projects-actions">
            <el-button :icon="Refresh" size="small" text :disabled="running || loading" aria-label="刷新项目配置" @click="loadProjects" />
            <el-button :icon="Plus" size="small" text :disabled="running" @click="newProject">新建</el-button>
          </div>
        </div>
        <div v-if="projects.length === 0" class="projects-empty">暂无项目配置</div>
        <button
          v-for="project in projects"
          :key="project.id"
          type="button"
          class="project-item"
          :class="{ active: project.id === selectedId }"
          :disabled="running"
          @click="selectProject(project)"
        >
          <span class="project-name">{{ project.name }}</span>
          <span class="project-updated">{{ project.updatedAt || "未保存" }}</span>
        </button>
      </aside>

      <main class="release-package-editor">
        <el-form label-position="top" class="release-package-form">
          <section class="project-overview">
            <header class="editor-header">
              <div class="editor-title">
                <el-input
                  v-if="titleEditing"
                  ref="projectTitleInput"
                  v-model="draft.name"
                  class="project-title-input"
                  :disabled="running"
                  aria-label="项目名称"
                  placeholder="请输入项目名称"
                  @blur="finishTitleEdit"
                  @keydown.enter.stop.prevent="finishTitleEdit"
                  @keydown.esc.stop.prevent="finishTitleEdit"
                />
                <h2 v-else>
                  <button
                    type="button"
                    class="project-title"
                    :disabled="running"
                    title="双击编辑项目标题"
                    @dblclick="startTitleEdit"
                    @keydown.enter.prevent="startTitleEdit"
                  >
                    {{ draft.name || "新建上线包项目" }}
                  </button>
                </h2>
              </div>
              <div class="editor-actions">
                <el-button v-if="selectedProject" :icon="Delete" type="danger" text :disabled="running || saving" @click="deleteProject">
                  删除配置
                </el-button>
                <el-button :icon="DocumentChecked" :loading="saving" :disabled="running" @click="saveProject">保存配置</el-button>
                <el-button :icon="VideoPlay" type="primary" :disabled="running || !selectedProject || dirty" @click="prepareStart">开始打包</el-button>
                <el-button v-if="running" :icon="VideoPause" type="danger" @click="cancelRun">终止打包</el-button>
                <el-button v-else-if="hasCommittedArchive" :icon="FolderOpened" @click="openArchive">打开归档目录</el-button>
              </div>
            </header>

            <div class="project-basics">
              <div class="project-basics-grid">
                <el-form-item label="打包类型" required>
                  <el-radio-group v-model="draft.packageType" :disabled="running" class="package-type-group">
                    <el-radio-button value="local_archive">本地归档</el-radio-button>
                    <el-radio-button value="server_upload">上传服务器</el-radio-button>
                  </el-radio-group>
                </el-form-item>
                <el-form-item v-if="draft.packageType === 'local_archive'" label="归档根目录" required>
                  <el-input v-model="draft.outputRoot" :disabled="running" placeholder="当前项目的上线包归档目录" readonly>
                    <template #append>
                      <el-button :icon="FolderOpened" :disabled="running" @click="chooseOutputRoot">选择</el-button>
                    </template>
                  </el-input>
                </el-form-item>
              </div>
            </div>
          </section>

          <div class="engineering-grid">
            <section class="engineering-card frontend-card">
              <header class="engineering-card-header">
                <div>
                  <span class="engineering-kicker">FRONTEND</span>
                  <h3>前端工程</h3>
                </div>
                <span class="engineering-index">01</span>
              </header>

              <el-form-item label="工程目录" required>
                <el-input v-model="draft.frontendProjectPath" :disabled="running" placeholder="前端工程绝对路径">
                  <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseFrontendProject">选择</el-button></template>
                </el-input>
              </el-form-item>
              <el-form-item required>
                <template #label>
                  <div class="command-label-row">
                    <span>构建命令</span>
                    <el-popover
                      placement="bottom-start"
                      trigger="click"
                      :width="440"
                      :teleported="true"
                      popper-class="release-package-command-examples"
                    >
                      <template #reference>
                        <el-button type="primary" text size="small">常用示例</el-button>
                      </template>
                      <div class="command-example-list">
                        <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example-item">
                          <div class="command-example-heading">
                            <strong>{{ example.title }}</strong>
                            <el-button
                              :icon="CopyDocument"
                              :aria-label="`复制${example.title}命令`"
                              size="small"
                              @click="copyCommandExample(example.command)"
                            >
                              复制
                            </el-button>
                          </div>
                          <p>{{ example.description }}</p>
                          <pre>{{ example.command }}</pre>
                        </article>
                      </div>
                    </el-popover>
                  </div>
                </template>
                <el-input
                  v-model="draft.frontendBuildCommand"
                  class="command-input"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 9 }"
                  :disabled="running"
                  placeholder="例如：pnpm build"
                />
                <p class="command-hint">多行命令将在同一 PowerShell 会话中顺序执行，前面设置的环境变量可在后续命令中复用。</p>
              </el-form-item>
              <el-form-item label="成功日志关键字（可选）">
                <el-input
                  v-model="draft.frontendSuccessKeyword"
                  :disabled="running"
                  placeholder="例如：Build completed"
                />
                <p class="command-hint">同时匹配 stdout 和 stderr，区分大小写；留空不检测。</p>
              </el-form-item>
              <div class="artifact-grid">
                <el-form-item label="产物路径" required>
                  <el-input v-model="draft.frontendArtifactPath" :disabled="running" placeholder="相对工程目录或绝对目录路径">
                    <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseFrontendArtifact">选择目录</el-button></template>
                  </el-input>
                </el-form-item>
                <el-form-item v-if="draft.packageType === 'local_archive'" label="本地归档处理" required>
                  <el-select v-model="draft.frontendArtifactMode" :disabled="running" class="full-width">
                    <el-option label="直接复制目录" value="copy_directory" />
                    <el-option label="压缩为 ZIP" value="zip_directory" />
                  </el-select>
                </el-form-item>
              </div>
            </section>

            <section class="engineering-card backend-card">
              <header class="engineering-card-header">
                <div>
                  <span class="engineering-kicker">BACKEND</span>
                  <h3>后端工程</h3>
                </div>
                <span class="engineering-index">02</span>
              </header>

              <el-form-item label="工程目录" required>
                <el-input v-model="draft.backendProjectPath" :disabled="running" placeholder="后端工程绝对路径">
                  <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseBackendProject">选择</el-button></template>
                </el-input>
              </el-form-item>
              <el-form-item required>
                <template #label>
                  <div class="command-label-row">
                    <span>构建命令</span>
                    <el-popover
                      placement="bottom-start"
                      trigger="click"
                      :width="440"
                      :teleported="true"
                      popper-class="release-package-command-examples"
                    >
                      <template #reference>
                        <el-button type="primary" text size="small">常用示例</el-button>
                      </template>
                      <div class="command-example-list">
                        <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example-item">
                          <div class="command-example-heading">
                            <strong>{{ example.title }}</strong>
                            <el-button
                              :icon="CopyDocument"
                              :aria-label="`复制${example.title}命令`"
                              size="small"
                              @click="copyCommandExample(example.command)"
                            >
                              复制
                            </el-button>
                          </div>
                          <p>{{ example.description }}</p>
                          <pre>{{ example.command }}</pre>
                        </article>
                      </div>
                    </el-popover>
                  </div>
                </template>
                <el-input
                  v-model="draft.backendBuildCommand"
                  class="command-input"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 9 }"
                  :disabled="running"
                  placeholder="例如：mvn clean package"
                />
                <p class="command-hint">
                  多行命令将在同一 PowerShell 会话中顺序执行，环境变量可复用；关键外部工具失败后请检查 $LASTEXITCODE。
                </p>
              </el-form-item>
              <el-form-item label="成功日志关键字（可选）">
                <el-input
                  v-model="draft.backendSuccessKeyword"
                  :disabled="running"
                  placeholder="例如：BUILD SUCCESS"
                />
                <p class="command-hint">同时匹配 stdout 和 stderr，区分大小写；留空不检测。</p>
              </el-form-item>
              <el-form-item label="产物路径" required>
                <el-input v-model="draft.backendArtifactPath" :disabled="running" placeholder="相对工程目录或绝对文件路径">
                  <template #append><el-button :icon="Document" :disabled="running" @click="chooseBackendArtifact">选择文件</el-button></template>
                </el-input>
              </el-form-item>
            </section>
          </div>

          <el-collapse v-if="draft.packageType === 'server_upload'" v-model="serverConfigSections" class="server-config-collapse">
            <el-collapse-item name="server">
              <template #title>
                <div class="server-config-heading">
                  <div>
                    <strong>Linux 服务器上传</strong>
                    <span>通过 SSH/SFTP 将构建产物内容替换到远程目标</span>
                  </div>
                  <el-tag type="success" effect="plain" size="small">当前类型</el-tag>
                </div>
              </template>

              <div class="server-config-body">
                <section class="server-config-section server-auth-section">
                  <div class="server-config-section-heading">
                    <div>
                      <strong>连接认证</strong>
                      <span>选择连接凭据，切换时只更新认证详情</span>
                    </div>
                  </div>

                  <div class="server-auth-type-row">
                    <el-form-item label="认证方式" required>
                      <el-radio-group v-model="draft.sshAuthType" :disabled="running" class="auth-type-group">
                        <el-radio-button value="password">账户密码</el-radio-button>
                        <el-radio-button value="private_key">私钥文件</el-radio-button>
                      </el-radio-group>
                    </el-form-item>
                  </div>

                  <div class="server-auth-details">
                    <div
                      v-if="draft.sshAuthType === 'password'"
                      class="server-auth-details-panel password-auth-panel"
                    >
                      <el-form-item
                        v-if="draft.sshAuthType === 'password'"
                        label="密码库凭据"
                        required
                        class="vault-credential-field"
                      >
                        <div class="vault-credential-picker">
                          <el-select
                            v-model="draft.vaultEntryId"
                            :disabled="running"
                            :loading="vaultOptionsLoading"
                            filterable
                            clearable
                            class="full-width"
                            placeholder="选择服务器凭据"
                          >
                            <el-option
                              v-for="option in vaultServerOptions"
                              :key="option.id"
                              :label="vaultCredentialLabel(option)"
                              :value="option.id"
                              :disabled="!option.complete"
                            />
                          </el-select>
                          <el-button :icon="Refresh" :loading="vaultOptionsLoading" :disabled="running" @click="loadVaultServerOptions">刷新</el-button>
                          <el-button :disabled="running" @click="openVault">密码管理</el-button>
                        </div>
                        <p class="vault-credential-hint">密码由密码库提供，上线包配置只保存凭据引用，不保存或展示服务器密码。</p>
                        <div v-if="vaultBindingInvalid" class="vault-binding-invalid" role="alert">
                          绑定的密码库凭据已失效，请重新选择
                        </div>
                        <div v-else-if="selectedVaultCredential" class="vault-credential-summary">
                          <div>
                            <span>服务器地址</span>
                            <code>{{ selectedVaultCredential.address }}</code>
                          </div>
                          <div>
                            <span>SSH 端口</span>
                            <code>{{ selectedVaultCredential.port }}</code>
                          </div>
                          <div>
                            <span>SSH 用户名</span>
                            <code>{{ selectedVaultCredential.account }}</code>
                          </div>
                        </div>
                      </el-form-item>
                    </div>

                    <div
                      v-if="draft.sshAuthType === 'private_key'"
                      class="server-auth-details-panel private-key-auth-panel"
                    >
                      <div class="private-key-config-grid">
                        <el-form-item v-if="draft.sshAuthType === 'private_key'" label="服务器地址" required>
                          <el-input v-model="draft.sshHost" :disabled="running" placeholder="例如：10.0.0.8" />
                        </el-form-item>
                        <el-form-item v-if="draft.sshAuthType === 'private_key'" label="SSH 端口" required>
                          <el-input-number v-model="draft.sshPort" :disabled="running" :min="1" :max="65535" controls-position="right" class="full-width" />
                        </el-form-item>
                        <el-form-item v-if="draft.sshAuthType === 'private_key'" label="SSH 用户名" required>
                          <el-input v-model="draft.sshUsername" :disabled="running" placeholder="例如：deploy" />
                        </el-form-item>
                        <el-form-item v-if="draft.sshAuthType === 'private_key'" label="私钥文件" required class="private-key-file-field">
                          <el-input v-model="draft.sshPrivateKeyPath" :disabled="running" placeholder="选择 OpenSSH 私钥文件" readonly>
                            <template #append>
                              <el-button :icon="Document" :disabled="running" @click="choosePrivateKey">选择私钥</el-button>
                            </template>
                          </el-input>
                        </el-form-item>
                      </div>
                    </div>
                  </div>
                </section>

                <section class="server-config-section server-target-section">
                  <div class="server-config-section-heading">
                    <div>
                      <strong>远程目标</strong>
                      <span>认证方式切换不会改变目标位置</span>
                    </div>
                  </div>
                  <div class="server-target-grid">
                    <el-form-item label="前端远程目录" required>
                      <el-input v-model="draft.frontendRemoteDir" :disabled="running" placeholder="例如：/srv/portal/web" />
                    </el-form-item>
                    <el-form-item label="后端远程文件" required>
                      <el-input v-model="draft.backendRemotePath" :disabled="running" placeholder="例如：/srv/portal/app.jar" />
                    </el-form-item>
                  </div>
                  <div class="server-command-grid">
                    <el-form-item label="前端上传后命令（可选）">
                      <el-input
                        v-model="draft.frontendPostUploadCommand"
                        type="textarea"
                        :autosize="{ minRows: 3, maxRows: 8 }"
                        :disabled="running"
                        placeholder="例如：systemctl reload nginx"
                      />
                    </el-form-item>
                    <el-form-item label="后端上传后命令（可选）">
                      <el-input
                        v-model="draft.backendPostUploadCommand"
                        type="textarea"
                        :autosize="{ minRows: 3, maxRows: 8 }"
                        :disabled="running"
                        placeholder="例如：systemctl restart portal"
                      />
                    </el-form-item>
                  </div>
                  <p class="server-command-note">全部选中目标上传成功后执行；不自动注入 sudo、工作目录或路径变量。</p>
                </section>
              </div>
            </el-collapse-item>
          </el-collapse>
        </el-form>

        <section v-if="selectedProject" class="release-package-log-card release-package-project-log">
          <header class="log-card-header">
            <div>
              <h3>{{ selectedProject.name }} · 运行日志</h3>
              <p>日志归属于当前项目，前后端任务独立执行和滚动。</p>
            </div>
            <el-tag
              class="log-status"
              role="status"
              aria-live="polite"
              aria-atomic="true"
              :type="statusTagTypes[status]"
              effect="plain"
              size="small"
            >
              {{ statusLabel }}
            </el-tag>
            <div
              v-if="overallError"
              class="log-error-summary log-overall-error"
              role="alert"
              :class="{ warning: status === 'succeeded' || status === 'partially_succeeded' }"
            >
              {{ overallError }}
            </div>
          </header>
          <div class="release-package-log-columns" :class="{ 'has-upload-lane': draft.packageType === 'server_upload' }">
            <section class="release-package-log-lane">
              <header class="log-lane-header">
                <strong>前端</strong>
                <div class="log-lane-actions">
                  <el-tag size="small" effect="plain" :type="targetStatusTagTypes[frontendStatus]">
                    {{ targetStatusLabels[frontendStatus] }}
                  </el-tag>
                  <el-button
                    v-if="hasCommittedArchive"
                    :icon="FolderOpened"
                    size="small"
                    text
                    :disabled="running"
                    aria-label="打开归档目录"
                    @click="openArchive"
                  />
                </div>
              </header>
              <div v-if="frontendError" class="log-error-summary log-lane-error" role="alert">
                {{ frontendError }}
              </div>
              <div ref="frontendLogContainer" class="release-package-log" aria-live="polite" aria-label="前端打包日志">
                <div v-if="frontendLogs.length === 0" class="log-empty">暂无前端日志</div>
                <div v-for="(entry, index) in frontendLogs" :key="`${entry.runId}-frontend-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
                  <span class="log-meta">[{{ entry.stream }}]</span>
                  <span>{{ entry.line }}</span>
                </div>
              </div>
            </section>
            <section class="release-package-log-lane">
              <header class="log-lane-header">
                <strong>后端</strong>
                <div class="log-lane-actions">
                  <el-tag size="small" effect="plain" :type="targetStatusTagTypes[backendStatus]">
                    {{ targetStatusLabels[backendStatus] }}
                  </el-tag>
                  <el-button
                    v-if="hasCommittedArchive"
                    :icon="FolderOpened"
                    size="small"
                    text
                    :disabled="running"
                    aria-label="打开归档目录"
                    @click="openArchive"
                  />
                </div>
              </header>
              <div v-if="backendError" class="log-error-summary log-lane-error" role="alert">
                {{ backendError }}
              </div>
              <div ref="backendLogContainer" class="release-package-log" aria-live="polite" aria-label="后端打包日志">
                <div v-if="backendLogs.length === 0" class="log-empty">暂无后端日志</div>
                <div v-for="(entry, index) in backendLogs" :key="`${entry.runId}-backend-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
                  <span class="log-meta">[{{ entry.stream }}]</span>
                  <span>{{ entry.line }}</span>
                </div>
              </div>
            </section>
            <section v-if="draft.packageType === 'server_upload'" class="release-package-log-lane upload-log-lane">
              <header class="log-lane-header upload-lane-header">
                <div class="upload-lane-title">
                  <strong>上传日志</strong>
                  <span v-if="uploadProgress.currentPath" class="upload-current-path">{{ uploadProgress.currentPath }}</span>
                </div>
                <div class="upload-lane-actions">
                  <div class="command-status-tags" aria-label="上传后命令状态">
                    <el-tag
                      size="small"
                      effect="plain"
                      :type="commandStatusTagTypes[frontendCommandStatus]"
                      :title="frontendCommandError || undefined"
                    >
                      前端命令 · {{ commandStatusLabels[frontendCommandStatus] }}
                    </el-tag>
                    <el-tag
                      size="small"
                      effect="plain"
                      :type="commandStatusTagTypes[backendCommandStatus]"
                      :title="backendCommandError || undefined"
                    >
                      后端命令 · {{ commandStatusLabels[backendCommandStatus] }}
                    </el-tag>
                  </div>
                  <el-button
                    v-if="status === 'package_succeeded_upload_failed' && retryToken"
                    :icon="Refresh"
                    size="small"
                    type="primary"
                    plain
                    :disabled="running"
                    @click="prepareUploadRetry"
                  >
                    重试上传
                  </el-button>
                  <el-button
                    v-else-if="status === 'upload_succeeded_command_failed' && commandRetryToken"
                    :icon="Refresh"
                    size="small"
                    type="warning"
                    plain
                    :disabled="running"
                    @click="prepareCommandRetry"
                  >
                    仅重试失败命令
                  </el-button>
                </div>
              </header>
              <div
                v-if="overallError"
                class="log-error-summary log-lane-error"
                role="alert"
                :class="{ warning: status === 'succeeded' || status === 'partially_succeeded' }"
              >
                {{ overallError }}
              </div>
              <div v-if="uploadProgress.totalBytes > 0" class="upload-progress" aria-live="polite">
                <el-progress :percentage="uploadPercentage" :stroke-width="6" />
                <span>{{ formatUploadBytes(uploadProgress.uploadedBytes) }} / {{ formatUploadBytes(uploadProgress.totalBytes) }}</span>
              </div>
              <div ref="uploadLogContainer" class="release-package-log" aria-live="polite" aria-label="服务器上传日志">
                <div v-if="uploadLogs.length === 0" class="log-empty">暂无上传日志</div>
                <div v-for="(entry, index) in uploadLogs" :key="`${entry.runId}-upload-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
                  <span class="log-meta">[{{ entry.stream }}]</span>
                  <span>{{ entry.line }}</span>
                </div>
              </div>
            </section>
          </div>
        </section>
      </main>
    </div>

    <el-dialog
      v-model="confirmVisible"
      :title="retryMode ? '重试上传' : isUploadStart ? '确认上传' : '确认本地归档'"
      width="min(640px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="!starting"
      :show-close="!starting"
      :before-close="beforeCloseStartDialog"
      @closed="resetStartDialog"
    >
      <el-form label-position="top">
        <el-form-item v-if="isLocalArchiveStart" label="归档目录名" required>
          <el-input v-model="folderName" placeholder="例如：20260723-订单管理系统" />
        </el-form-item>
        <el-form-item v-if="isUploadStart && draft.sshAuthType === 'private_key'" label="私钥口令（可选）">
          <el-input
            v-model="credentialSecret"
            type="password"
            show-password
            autocomplete="new-password"
            :disabled="starting"
            placeholder="私钥未加密时可留空"
          />
        </el-form-item>
        <div v-if="isUploadStart && draft.sshAuthType === 'password'" class="vault-start-summary">
          <span>密码库凭据</span>
          <strong>{{ selectedVaultCredential?.title || `凭据 #${draft.vaultEntryId ?? "-"}` }}</strong>
          <p>密码由密码库提供，仅在已信任主机后由本地 Rust 进程读取。</p>
        </div>
      </el-form>
      <p v-if="isLocalArchiveStart" class="archive-preview">完整归档路径：{{ archivePathPreview || "请先设置归档根目录" }}</p>
      <div v-if="!retryMode" class="package-targets">
        <span class="package-targets-label">本次打包内容（默认全选）</span>
        <el-checkbox-group v-model="selectedTargets" :disabled="starting">
          <el-checkbox label="前端包" value="frontend" />
          <el-checkbox label="后端包" value="backend" />
        </el-checkbox-group>
      </div>
      <div v-if="isUploadStart && uploadPreflight.probeResult.value" class="preflight-summary">
        <div class="preflight-host">
          <span>主机指纹</span>
          <code>{{ uploadPreflight.probeResult.value.fingerprintSha256 }}</code>
        </div>
        <div v-if="uploadPreflight.preflightResult.value" class="preflight-targets">
          <div v-for="target in uploadPreflight.preflightResult.value.targets" :key="target.target" class="preflight-target-row">
            <span>{{ target.target === "frontend" ? "前端" : "后端" }}</span>
            <code>{{ target.remotePath }}</code>
            <el-tag :type="target.exists ? 'warning' : 'success'" effect="plain" size="small">
              {{ target.exists ? "将替换" : "新建" }}
            </el-tag>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button v-if="starting" type="danger" :disabled="cancelPendingStart" @click="cancelRun">
          {{ cancelPendingStart ? "等待终止" : retryMode ? "终止上传" : "终止打包" }}
        </el-button>
        <el-button v-else @click="closeStartDialog">取消</el-button>
        <el-button type="primary" :loading="starting" :disabled="starting" @click="confirmStart">
          {{ retryMode ? "确认重试" : isUploadStart ? "确认构建并上传" : "确认归档" }}
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="commandRetryVisible"
      title="重试上传后命令"
      width="min(560px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="!commandRetryStarting"
      :show-close="!commandRetryStarting"
      :before-close="beforeCloseCommandRetryDialog"
      @closed="resetCommandRetryDialog"
    >
      <p class="command-retry-notice" role="status">
        服务器文件已上传。本次只重新认证并执行明确失败的上传后命令，不会重新构建或上传文件。
      </p>
      <div v-if="commandRetry.prepareResult.value" class="command-retry-summary">
        <div>
          <span>服务器</span>
          <code>{{ commandRetry.prepareResult.value.host }}:{{ commandRetry.prepareResult.value.port }}</code>
        </div>
        <div>
          <span>账号</span>
          <code>{{ commandRetry.prepareResult.value.username }}</code>
        </div>
        <div>
          <span>失败目标</span>
          <strong>{{ commandRetryTargetLabel }}</strong>
        </div>
      </div>
      <el-form label-position="top">
        <el-form-item
          v-if="commandRetry.prepareResult.value?.authType === 'private_key'"
          label="私钥口令（可选）"
        >
          <el-input
            v-model="commandRetry.privateKeyPassphrase.value"
            type="password"
            show-password
            autocomplete="new-password"
            :disabled="commandRetryStarting"
            placeholder="请重新输入；私钥未加密时可留空"
          />
        </el-form-item>
        <div
          v-if="commandRetry.prepareResult.value?.authType === 'password'"
          class="vault-start-summary"
        >
          <span>密码库认证</span>
          <strong>{{ commandRetry.prepareResult.value.username }}@{{ commandRetry.prepareResult.value.host }}</strong>
          <p>密码由失败任务绑定的 Vault 服务器凭据提供，前端不会读取或保存密码。</p>
        </div>
      </el-form>
      <div v-if="commandRetry.prepareResult.value" class="preflight-summary">
        <div class="preflight-host">
          <span>主机指纹</span>
          <code>{{ commandRetry.prepareResult.value.fingerprintSha256 }}</code>
        </div>
      </div>
      <template #footer>
        <el-button :disabled="commandRetryStarting" @click="closeCommandRetryDialog">取消</el-button>
        <el-button
          type="primary"
          :loading="commandRetryStarting"
          :disabled="commandRetryStarting"
          @click="confirmCommandRetry"
        >
          仅重试失败命令
        </el-button>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, h, nextTick, onMounted, reactive, ref, watch } from "vue";
import { CopyDocument, Delete, Document, DocumentChecked, FolderOpened, Plus, Refresh, VideoPause, VideoPlay } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import type { InputInstance } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import { useActionDispatchIntent } from "../composables/useActionDispatchIntent";
import { useReleasePackageRuntime } from "../composables/useReleasePackageRuntime";
import { useReleasePackageCommandRetry } from "../composables/useReleasePackageCommandRetry";
import { useReleasePackageUploadPreflight } from "../composables/useReleasePackageUploadPreflight";
import type { ActionDispatchRequest } from "../types";
import type {
  ReleasePackageCommandStatus,
  ReleasePackagePrepareResult,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
  ReleasePackageRemoteProbeResult,
  ReleasePackageRunStatus,
  ReleasePackageStartResult,
  ReleasePackageTarget,
  ReleasePackageTargetCheckResult,
  ReleasePackageTargetStatus,
} from "../types/release-package";
import {
  RELEASE_PACKAGE_COMMAND_EXAMPLES,
  createDefaultReleasePackageTargets,
  createEmptyReleasePackageDraft,
  createReleasePackageStartPayload,
  isReleasePackageDraftDirty,
  normalizeVaultServerPort,
  projectToReleasePackageDraft,
  releasePackageRunStatusLabel,
  validateReleasePackageDraft,
  validateReleasePackageUpload,
  validateReleasePackageTargets,
  writeReleasePackageCommand,
} from "../utils/releasePackage";

interface VaultServerOption {
  id: number;
  title: string;
  environment: string;
  address: string;
  port: number | null;
  account: string;
  complete: boolean;
}

interface VaultMetaEntry {
  id: number;
  category: string;
  title: string;
  environment?: string;
  plainFields?: {
    address?: string;
    port?: unknown;
    account?: string;
  } | null;
}

const emit = defineEmits<{
  (event: "open-tool", toolId: string): void;
}>();

const projects = ref<ReleasePackageProject[]>([]);
const selectedId = ref<number | null>(null);
const draft = reactive<ReleasePackageProjectDraft>(createEmptyReleasePackageDraft());
const loading = ref(false);
const saving = ref(false);
const starting = ref(false);
const cancelPendingStart = ref(false);
const confirmVisible = ref(false);
const pendingActionDispatchId = ref<string | null>(null);
const prepareResult = ref<ReleasePackagePrepareResult | null>(null);
const folderName = ref("");
const selectedTargets = ref<ReleasePackageTarget[]>(createDefaultReleasePackageTargets());
const credentialSecret = ref("");
const overwriteRemoteTargets = ref<ReleasePackageTarget[]>([]);
const retryMode = ref(false);
const commandRetryVisible = ref(false);
const commandRetryStarting = ref(false);
const serverConfigSections = ref<string[]>([]);
const vaultServerOptions = ref<VaultServerOption[]>([]);
const vaultOptionsLoading = ref(false);
const vaultOptionsLoaded = ref(false);
const titleEditing = ref(false);
const projectTitleInput = ref<InputInstance | null>(null);
const frontendLogContainer = ref<HTMLElement | null>(null);
const backendLogContainer = ref<HTMLElement | null>(null);
const uploadLogContainer = ref<HTMLElement | null>(null);
const runtime = useReleasePackageRuntime();
const uploadPreflight = useReleasePackageUploadPreflight();
const commandRetry = useReleasePackageCommandRetry();
const { watchPendingIntent } = useActionDispatchIntent();
const statusTagTypes: Record<ReleasePackageRunStatus, "primary" | "success" | "info" | "warning" | "danger"> = {
  idle: "info",
  prechecking: "primary",
  running: "primary",
  uploading: "primary",
  succeeded: "success",
  partially_succeeded: "warning",
  package_succeeded_upload_failed: "danger",
  upload_succeeded_command_failed: "danger",
  failed: "danger",
  cancelled: "warning",
};
const targetStatusTagTypes: Record<ReleasePackageTargetStatus, "primary" | "success" | "info" | "warning" | "danger"> = {
  idle: "info",
  pending: "info",
  running: "primary",
  succeeded: "success",
  failed: "danger",
  cancelled: "warning",
  skipped: "info",
};
const targetStatusLabels: Record<ReleasePackageTargetStatus, string> = {
  idle: "未运行",
  pending: "等待中",
  running: "运行中",
  succeeded: "成功",
  failed: "失败",
  cancelled: "已终止",
  skipped: "未选择",
};
const commandStatusTagTypes: Record<ReleasePackageCommandStatus, "primary" | "success" | "info" | "warning" | "danger"> = {
  pending: "info",
  running: "primary",
  succeeded: "success",
  failed: "danger",
  cancelled: "warning",
  skipped: "info",
};
const commandStatusLabels: Record<ReleasePackageCommandStatus, string> = {
  pending: "等待中",
  running: "执行中",
  succeeded: "成功",
  failed: "失败",
  cancelled: "已终止",
  skipped: "未配置",
};

const selectedProject = computed(() => projects.value.find((item) => item.id === selectedId.value) ?? null);
const dirty = computed(() => isReleasePackageDraftDirty(selectedProject.value, draft));
const currentProjectRuntime = computed(() => selectedId.value === null ? null : runtime.getProjectRuntime(selectedId.value));
const status = computed<ReleasePackageRunStatus>(() => currentProjectRuntime.value?.status ?? "idle");
const archivePath = computed(() => currentProjectRuntime.value?.archivePath ?? "");
const frontendLogs = computed(() => currentProjectRuntime.value?.frontendLogs ?? []);
const backendLogs = computed(() => currentProjectRuntime.value?.backendLogs ?? []);
const uploadLogs = computed(() => currentProjectRuntime.value?.uploadLogs ?? []);
const uploadProgress = computed(() => currentProjectRuntime.value?.uploadProgress ?? {
  uploadedBytes: 0,
  totalBytes: 0,
  currentPath: "",
});
const retryToken = computed(() => currentProjectRuntime.value?.retryToken ?? "");
const commandRetryToken = computed(() => currentProjectRuntime.value?.commandRetryToken ?? "");
const overallError = computed(() => currentProjectRuntime.value?.error ?? "");
const frontendError = computed(() => currentProjectRuntime.value?.targetErrors.frontend ?? "");
const backendError = computed(() => currentProjectRuntime.value?.targetErrors.backend ?? "");
const frontendCommandStatus = computed<ReleasePackageCommandStatus>(
  () => currentProjectRuntime.value?.commandStatus.frontend ?? "skipped",
);
const backendCommandStatus = computed<ReleasePackageCommandStatus>(
  () => currentProjectRuntime.value?.commandStatus.backend ?? "skipped",
);
const frontendCommandError = computed(() => currentProjectRuntime.value?.commandErrors.frontend ?? "");
const backendCommandError = computed(() => currentProjectRuntime.value?.commandErrors.backend ?? "");
const commandRetryTargetLabel = computed(() => {
  const targets = commandRetry.prepareResult.value?.targets ?? [];
  return targets.map((target) => target === "frontend" ? "前端" : "后端").join("、") || "-";
});
const frontendStatus = computed<ReleasePackageTargetStatus>(() => currentProjectRuntime.value?.targetStatus.frontend ?? "idle");
const backendStatus = computed<ReleasePackageTargetStatus>(() => currentProjectRuntime.value?.targetStatus.backend ?? "idle");
const running = runtime.isRunning;
const statusLabel = computed(() => releasePackageRunStatusLabel(status.value));
const hasCommittedArchive = computed(() => Boolean(archivePath.value) && [
  "succeeded",
  "partially_succeeded",
  "package_succeeded_upload_failed",
].includes(status.value));
const uploadPercentage = computed(() => {
  if (uploadProgress.value.totalBytes <= 0) return 0;
  return Math.min(100, Math.round(
    uploadProgress.value.uploadedBytes / uploadProgress.value.totalBytes * 100,
  ));
});
const selectedVaultCredential = computed(() => vaultServerOptions.value.find(
  (option) => option.id === draft.vaultEntryId,
) ?? null);
const vaultBindingInvalid = computed(() => (
  draft.sshAuthType === "password"
  && draft.vaultEntryId !== null
  && vaultOptionsLoaded.value
  && (!selectedVaultCredential.value || !selectedVaultCredential.value.complete)
));
const isUploadStart = computed(() => retryMode.value || prepareResult.value?.packageType === "server_upload");
const isLocalArchiveStart = computed(() => !retryMode.value && prepareResult.value?.packageType === "local_archive");
const archivePathPreview = computed(() => {
  if (prepareResult.value?.packageType !== "local_archive") return "";
  const preparedRoot = prepareResult.value.outputRoot;
  if (!preparedRoot || !folderName.value) return "";
  if (folderName.value === prepareResult.value.defaultFolderName) {
    return prepareResult.value.archivePath;
  }
  return `${preparedRoot.replace(/[\\/]+$/, "")}/${folderName.value}`;
});

function showError(error: unknown): void {
  ElMessage.error(error instanceof Error ? error.message : String(error));
}

function vaultCredentialLabel(option: VaultServerOption): string {
  const suffix = [option.environment, option.account, option.address].filter(Boolean).join(" · ");
  return suffix ? `${option.title} · ${suffix}` : option.title;
}

async function loadVaultServerOptions(): Promise<void> {
  if (vaultOptionsLoading.value) return;
  vaultOptionsLoading.value = true;
  try {
    const result = await invokeToolByChannel("tool:vault:meta-list", { category: "server" }) as VaultMetaEntry[];
    if (!Array.isArray(result)) throw new Error("密码库服务器凭据列表格式无效");
    vaultServerOptions.value = result
      .filter((entry) => entry.category === "server")
      .map((entry) => {
        const address = entry.plainFields?.address?.trim() ?? "";
        const port = normalizeVaultServerPort(entry.plainFields?.port);
        const account = entry.plainFields?.account?.trim() ?? "";
        return {
          id: entry.id,
          title: entry.title || `(未命名凭据 #${entry.id})`,
          environment: entry.environment?.trim() ?? "",
          address,
          port,
          account,
          complete: Boolean(address && account && port !== null),
        };
      });
    vaultOptionsLoaded.value = true;
  } catch (error) {
    showError(error);
  } finally {
    vaultOptionsLoading.value = false;
  }
}

function openVault(): void {
  emit("open-tool", "vault");
}

async function handleUploadIntegrationError(error: unknown): Promise<void> {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("vault_locked")) {
    try {
      await ElMessageBox.confirm(
        "密码库当前已锁定。请先打开密码管理并解锁，再重新发起上传。",
        "需要解锁密码库",
        {
          type: "warning",
          confirmButtonText: "打开密码管理",
          cancelButtonText: "稍后处理",
        },
      );
      openVault();
    } catch (confirmError) {
      if (confirmError !== "cancel" && confirmError !== "close") showError(confirmError);
    }
    return;
  }
  if (message.includes("vault_entry_not_found")) {
    ElMessage.error("绑定的密码库凭据不存在，请重新选择并保存配置");
    return;
  }
  if (message.includes("vault_entry_invalid_category")) {
    ElMessage.error("绑定的密码库条目不是服务器凭据，请重新选择");
    return;
  }
  if (message.includes("vault_entry_incomplete")) {
    ElMessage.error("绑定的服务器凭据缺少地址、端口、账号或密码，请在密码管理中补充");
    return;
  }
  showError(error);
}

async function startTitleEdit(): Promise<void> {
  if (running.value) return;
  titleEditing.value = true;
  await nextTick();
  projectTitleInput.value?.focus();
  projectTitleInput.value?.select();
}

function finishTitleEdit(): void {
  titleEditing.value = false;
}

async function copyCommandExample(command: string): Promise<void> {
  try {
    await writeReleasePackageCommand(command, (value) => navigator.clipboard.writeText(value));
    ElMessage.success("命令示例已复制");
  } catch (error) {
    showError(error);
  }
}

async function loadProjects(
  options: { preserveEditor?: boolean } = {},
): Promise<boolean> {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:release-package:project-list", {})) as { projects?: ReleasePackageProject[] };
    projects.value = result.projects ?? [];
    if (options.preserveEditor) return true;
    const current = projects.value.find((project) => project.id === selectedId.value);
    const active = projects.value.find((project) => project.id === runtime.activeProjectId.value);
    const preferActiveProject = (selectedId.value === null && !dirty.value) || runtime.status.value === "running" || runtime.status.value === "uploading";
    const preserveUnsavedDraft = selectedId.value === null && dirty.value && !preferActiveProject;
    const target = preferActiveProject ? active ?? current ?? projects.value[0] : current;
    if (target) {
      const selectionChanged = selectedId.value !== target.id;
      selectedId.value = target.id;
      if (selectionChanged || !dirty.value) Object.assign(draft, projectToReleasePackageDraft(target));
    } else if (!preserveUnsavedDraft) {
      selectedId.value = null;
      Object.assign(draft, createEmptyReleasePackageDraft());
    }
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    loading.value = false;
  }
}

async function confirmDiscardChanges(): Promise<boolean> {
  if (!dirty.value) return true;
  try {
    await ElMessageBox.confirm("当前有未保存的修改，直接切换将丢失这些修改。", "未保存的修改", { type: "warning" });
    return true;
  } catch {
    return false;
  }
}

async function selectProject(project: ReleasePackageProject): Promise<void> {
  if (project.id === selectedId.value || !(await confirmDiscardChanges())) return;
  selectedId.value = project.id;
  Object.assign(draft, projectToReleasePackageDraft(project));
}

async function newProject(): Promise<void> {
  if (!(await confirmDiscardChanges())) return;
  selectedId.value = null;
  Object.assign(draft, createEmptyReleasePackageDraft());
}

async function saveProject(): Promise<void> {
  const validationError = validateReleasePackageDraft(draft);
  if (validationError) {
    ElMessage.warning(validationError);
    return;
  }
  saving.value = true;
  try {
    const payload = { ...draft };
    const channel = selectedId.value ? "tool:release-package:project-update" : "tool:release-package:project-create";
    const result = (await invokeToolByChannel(channel, selectedId.value ? { id: selectedId.value, ...payload } : payload)) as { id?: number };
    const savedId = result.id ?? selectedId.value;
    if (savedId) selectedId.value = savedId;
    const refreshed = await loadProjects();
    if (!refreshed) return;
    if (savedId) {
      selectedId.value = savedId;
      const saved = projects.value.find((project) => project.id === savedId);
      if (saved) Object.assign(draft, projectToReleasePackageDraft(saved));
    }
    ElMessage.success("项目配置已保存");
  } catch (error) {
    showError(error);
  } finally {
    saving.value = false;
  }
}

async function deleteProject(): Promise<void> {
  const project = selectedProject.value;
  if (!project) return;
  try {
    await ElMessageBox.confirm(
      `确定删除「${project.name}」的配置吗？只删除配置，不删除工程或归档文件。`,
      "删除项目配置",
      { type: "warning" },
    );
    await invokeToolByChannel("tool:release-package:project-delete", { id: project.id });
    projects.value = projects.value.filter((item) => item.id !== project.id);
    selectedId.value = null;
    Object.assign(draft, createEmptyReleasePackageDraft());
    const refreshed = await loadProjects();
    if (!refreshed) return;
    ElMessage.success("项目配置已删除");
  } catch (error) {
    if (error !== "cancel" && error !== "close") showError(error);
  }
}

async function chooseDirectory(title: string): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title });
  return typeof selected === "string" ? selected : null;
}

async function chooseFile(title: string): Promise<string | null> {
  const selected = await open({ directory: false, multiple: false, title });
  return typeof selected === "string" ? selected : null;
}

async function chooseOutputRoot(): Promise<void> {
  try {
    const path = await chooseDirectory("选择归档根目录");
    if (!path) return;
    draft.outputRoot = path;
  } catch (error) {
    showError(error);
  }
}

async function chooseFrontendProject(): Promise<void> {
  try {
    const path = await chooseDirectory("选择前端工程目录");
    if (path) draft.frontendProjectPath = path;
  } catch (error) {
    showError(error);
  }
}

async function chooseBackendProject(): Promise<void> {
  try {
    const path = await chooseDirectory("选择后端工程目录");
    if (path) draft.backendProjectPath = path;
  } catch (error) {
    showError(error);
  }
}

async function chooseFrontendArtifact(): Promise<void> {
  try {
    const path = await chooseDirectory("选择前端产物目录");
    if (path) draft.frontendArtifactPath = path;
  } catch (error) {
    showError(error);
  }
}

async function chooseBackendArtifact(): Promise<void> {
  try {
    const path = await chooseFile("选择后端产物文件");
    if (path) draft.backendArtifactPath = path;
  } catch (error) {
    showError(error);
  }
}

async function choosePrivateKey(): Promise<void> {
  try {
    const path = await chooseFile("选择 SSH 私钥文件");
    if (path) draft.sshPrivateKeyPath = path;
  } catch (error) {
    showError(error);
  }
}

async function clearSensitiveStartState(): Promise<void> {
  credentialSecret.value = "";
  overwriteRemoteTargets.value = [];
  try {
    await uploadPreflight.reset();
  } catch (error) {
    showError(error);
  }
}

async function resetStartDialog(): Promise<void> {
  retryMode.value = false;
  await clearSensitiveStartState();
}

async function stopPendingActionDispatch(
  outcome: "cancelled" | "failed",
  error?: string,
): Promise<void> {
  const dispatchId = pendingActionDispatchId.value;
  if (!dispatchId) return;
  await invokeToolByChannel("tool:action-center:dispatch-cancel", {
    dispatchId,
    outcome,
    ...(error ? { error } : {}),
  });
  pendingActionDispatchId.value = null;
}

async function closeStartDialog(): Promise<void> {
  await stopPendingActionDispatch("cancelled");
  confirmVisible.value = false;
  await clearSensitiveStartState();
}

async function beforeCloseStartDialog(done: () => void): Promise<void> {
  if (starting.value) return;
  try {
    await stopPendingActionDispatch("cancelled");
    done();
  } catch (error) {
    showError(error);
  }
}

async function prepareStart(): Promise<Error | null> {
  if (!selectedProject.value || dirty.value) {
    const error = new Error(dirty.value ? "请先保存项目配置" : "请先选择项目");
    ElMessage.warning(error.message);
    return error;
  }
  if (running.value) return new Error("已有发布打包任务正在运行");
  selectedTargets.value = createDefaultReleasePackageTargets();
  try {
    await resetStartDialog();
    prepareResult.value = (await invokeToolByChannel("tool:release-package:prepare", {
      projectId: selectedProject.value.id,
    })) as ReleasePackagePrepareResult;
    folderName.value = prepareResult.value.packageType === "local_archive"
      ? prepareResult.value.defaultFolderName
      : "";
    confirmVisible.value = true;
    return null;
  } catch (error) {
    showError(error);
    return error instanceof Error ? error : new Error(String(error));
  }
}

async function applyActionDispatchIntent(intent: ActionDispatchRequest): Promise<void> {
  if (intent.actionType !== "release_package.run") return;
  pendingActionDispatchId.value = intent.dispatchId;
  try {
    if (dirty.value) {
      await stopPendingActionDispatch("failed", "上线包页面有未保存配置，未切换打包项目");
      ElMessage.error("上线包页面有未保存配置，动作已停止");
      return;
    }
    if (running.value) {
      await stopPendingActionDispatch("failed", "已有发布打包任务正在运行");
      ElMessage.error("已有发布打包任务正在运行，动作已停止");
      return;
    }
    const loaded = await loadProjects({ preserveEditor: true });
    if (!loaded) {
      await stopPendingActionDispatch("failed", "加载上线包配置失败");
      return;
    }
    const target = projects.value.find((project) => String(project.id) === intent.targetId);
    if (!target) {
      await stopPendingActionDispatch("failed", "上线包配置不存在");
      ElMessage.error("上线包配置不存在，动作已停止");
      return;
    }
    selectedId.value = target.id;
    Object.assign(draft, projectToReleasePackageDraft(target));
    const prepareError = await prepareStart();
    if (prepareError) {
      await stopPendingActionDispatch("failed", prepareError.message);
    }
  } catch (error) {
    showError(error);
  }
}

function retryUploadTargets(): ReleasePackageTarget[] {
  const projectRuntime = currentProjectRuntime.value;
  if (!projectRuntime) return [];
  return (["frontend", "backend"] as const).filter(
    (target) => projectRuntime.targetStatus[target] !== "skipped",
  );
}

async function prepareUploadRetry(): Promise<void> {
  if (!selectedProject.value || !retryToken.value || running.value) return;
  await resetStartDialog();
  retryMode.value = true;
  prepareResult.value = { packageType: "server_upload" };
  selectedTargets.value = retryUploadTargets();
  confirmVisible.value = true;
}

async function resetCommandRetryDialog(): Promise<void> {
  commandRetryStarting.value = false;
  try {
    await commandRetry.reset();
  } catch (error) {
    showError(error);
  }
}

async function closeCommandRetryDialog(): Promise<void> {
  commandRetryVisible.value = false;
  await resetCommandRetryDialog();
}

async function beforeCloseCommandRetryDialog(done: () => void): Promise<void> {
  if (commandRetryStarting.value) return;
  try {
    await commandRetry.reset();
    done();
  } catch (error) {
    showError(error);
  }
}

async function prepareCommandRetry(): Promise<void> {
  const projectId = selectedProject.value?.id;
  if (!projectId || !commandRetryToken.value || running.value) return;
  try {
    await commandRetry.prepare(projectId, commandRetryToken.value);
    commandRetryVisible.value = true;
  } catch (error) {
    await handleUploadIntegrationError(error);
  }
}

async function confirmCommandRetry(): Promise<void> {
  const projectId = selectedProject.value?.id;
  const prepared = commandRetry.prepareResult.value;
  if (!projectId || !prepared) {
    ElMessage.warning("命令重试信息已失效，请关闭窗口后重新发起");
    return;
  }
  const commandTargets = [...prepared.targets];
  commandRetryStarting.value = true;
  let runtimeStartBegun = false;
  try {
    if (!(await ensureCommandRetryHostTrusted())) {
      commandRetry.privateKeyPassphrase.value = "";
      return;
    }
    await commandRetry.preflight();
    await runtime.ensureListeners();
    runtime.beginStart(projectId, commandTargets);
    runtimeStartBegun = true;
    const result = await commandRetry.start();
    runtime.bindStartedRun(result.runId, projectId);
    commandRetryVisible.value = false;
  } catch (error) {
    if (runtimeStartBegun) {
      runtime.abortStart(error instanceof Error ? error.message : String(error));
    }
    await handleUploadIntegrationError(error);
  } finally {
    commandRetryStarting.value = false;
  }
}
async function confirmArchiveOverwrite(projectId: number): Promise<boolean | null> {
  const target = (await invokeToolByChannel("tool:release-package:target-check", {
    projectId,
    folderName: folderName.value,
  })) as ReleasePackageTargetCheckResult;
  if (!target.exists) return false;
  try {
    await ElMessageBox.confirm(
      "目标归档目录已存在。直接覆盖将完整替换其中的所有文件，此操作无法撤销。",
      "目标归档目录已存在",
      {
        type: "warning",
        confirmButtonText: "直接覆盖",
        cancelButtonText: "取消",
      },
    );
    return true;
  } catch (error) {
    if (error === "cancel" || error === "close") return null;
    throw error;
  }
}

function hostTrustMessage(probe: ReleasePackageRemoteProbeResult) {
  const rows = [
    ["服务器", `${probe.host}:${probe.port}`],
    ["密钥类型", probe.keyType],
    ["当前指纹", probe.fingerprintSha256],
  ];
  if (probe.trust === "changed" && probe.previousFingerprintSha256) {
    rows.push(["原指纹", probe.previousFingerprintSha256]);
  }
  return h("div", { class: "release-package-host-trust" }, rows.map(([label, value]) =>
    h("div", { class: "host-trust-row" }, [
      h("span", label),
      h("code", value),
    ]),
  ));
}

async function confirmHostTrust(probe: ReleasePackageRemoteProbeResult): Promise<boolean> {
  if (probe.trust === "trusted") return true;
  try {
    await ElMessageBox.confirm(
      hostTrustMessage(probe),
      probe.trust === "changed" ? "SSH 主机指纹已变化" : "确认 SSH 主机指纹",
      {
        type: "warning",
        confirmButtonText: probe.trust === "changed" ? "更新信任并继续" : "信任并继续",
        cancelButtonText: "取消",
      },
    );
    return true;
  } catch (error) {
    if (error === "cancel" || error === "close") return false;
    throw error;
  }
}

async function ensureHostTrusted(projectId: number): Promise<boolean> {
  const probe = await uploadPreflight.probe(projectId);
  if (!probe || !(await confirmHostTrust(probe))) return false;
  if (probe.trust !== "trusted") {
    await uploadPreflight.trustHost(projectId, probe.trust === "changed");
  }
  return true;
}

async function ensureCommandRetryHostTrusted(): Promise<boolean> {
  const probe = commandRetry.prepareResult.value;
  if (!probe || !(await confirmHostTrust(probe))) return false;
  if (probe.trust !== "trusted") {
    await commandRetry.trustHost(probe.trust === "changed");
  }
  return true;
}

async function confirmRemoteOverwrite(): Promise<boolean> {
  const existingTargets = uploadPreflight.preflightResult.value?.targets.filter(
    (target) => target.exists,
  ) ?? [];
  overwriteRemoteTargets.value = existingTargets.map((target) => target.target);
  if (existingTargets.length === 0) return true;
  try {
    await ElMessageBox.confirm(
      h("div", { class: "release-package-remote-overwrite" }, [
        h("p", "完整替换以上远程目标"),
        h("ul", existingTargets.map((target) => h("li", [
          h("strong", target.target === "frontend" ? "前端：" : "后端："),
          h("code", target.remotePath),
        ]))),
        h("p", "替换失败时会尝试恢复原目标，但仍应确认服务器上没有并行发布。"),
      ]),
      "远程目标已存在",
      {
        type: "warning",
        confirmButtonText: "确认完整替换",
        cancelButtonText: "取消",
      },
    );
    return true;
  } catch (error) {
    overwriteRemoteTargets.value = [];
    if (error === "cancel" || error === "close") return false;
    throw error;
  }
}

async function runUploadPreflight(
  projectId: number,
  targets: ReleasePackageTarget[],
): Promise<boolean> {
  const uploadError = validateReleasePackageUpload(draft);
  if (uploadError) throw new Error(uploadError);
  if (draft.sshAuthType === "password" && draft.vaultEntryId === null) {
    throw new Error("请选择密码库服务器凭据");
  }
  if (!(await ensureHostTrusted(projectId))) return false;
  await uploadPreflight.check({
    projectId,
    targets: [...targets],
    ...(draft.sshAuthType === "private_key"
      ? { privateKeyPassphrase: credentialSecret.value || undefined }
      : {}),
  });
  return confirmRemoteOverwrite();
}

async function confirmStart(): Promise<void> {
  const projectId = selectedProject.value?.id;
  const targetsError = validateReleasePackageTargets(selectedTargets.value);
  const isRetry = retryMode.value;
  const packageType = isRetry ? "server_upload" : prepareResult.value?.packageType;
  if (packageType !== "local_archive" && packageType !== "server_upload") {
    ElMessage.warning("打包类型无效，请重新打开确认窗口");
    return;
  }
  const folderNameError = packageType === "local_archive"
    ? validateArchiveFolderName(folderName.value)
    : null;
  if (!projectId || folderNameError || targetsError) {
    ElMessage.warning(folderNameError ?? targetsError ?? "打包类型无效，请重新打开确认窗口");
    return;
  }
  starting.value = true;
  cancelPendingStart.value = false;
  let runtimeStartBegun = false;
  const retryTokenValue = retryToken.value;
  try {
    let overwriteExisting = false;
    if (packageType === "local_archive") {
      const overwriteDecision = await confirmArchiveOverwrite(projectId);
      if (overwriteDecision === null) {
        await stopPendingActionDispatch("cancelled");
        return;
      }
      overwriteExisting = overwriteDecision;
    } else if (packageType === "server_upload") {
      const preflightAccepted = await runUploadPreflight(projectId, selectedTargets.value);
      if (!preflightAccepted) {
        await stopPendingActionDispatch("cancelled");
        return;
      }
    }
    await runtime.ensureListeners();
    if (cancelPendingStart.value) {
      await stopPendingActionDispatch("cancelled");
      confirmVisible.value = false;
      ElMessage.info(isRetry ? "已取消上传" : "已取消打包");
      return;
    }
    runtime.beginStart(projectId, selectedTargets.value);
    runtimeStartBegun = true;
    const result = isRetry
      ? await invokeToolByChannel("tool:release-package:upload-retry", {
          projectId,
          retryToken: retryTokenValue,
          preflightToken: uploadPreflight.preflightToken.value,
          overwriteRemoteTargets: [...overwriteRemoteTargets.value],
        }) as ReleasePackageStartResult
      : await invokeToolByChannel(
          "tool:release-package:start",
          createReleasePackageStartPayload(packageType, {
            projectId,
            targets: selectedTargets.value,
            folderName: folderName.value,
            overwriteExisting,
            preflightToken: uploadPreflight.preflightToken.value,
            overwriteRemoteTargets: overwriteRemoteTargets.value,
            actionDispatchId: pendingActionDispatchId.value ?? undefined,
          }),
        ) as ReleasePackageStartResult;
    pendingActionDispatchId.value = null;
    runtime.bindStartedRun(result.runId, projectId);
    confirmVisible.value = false;
    if (cancelPendingStart.value) {
      try {
        await runtime.cancel();
        cancelPendingStart.value = false;
        ElMessage.info("已请求终止打包");
      } catch (error) {
        showError(error);
      }
    }
  } catch (error) {
    if (runtimeStartBegun) {
      runtime.abortStart(error instanceof Error ? error.message : String(error));
    }
    try {
      await stopPendingActionDispatch(
        "failed",
        error instanceof Error ? error.message : String(error),
      );
    } catch (dispatchError) {
      showError(dispatchError);
    }
    await handleUploadIntegrationError(error);
  } finally {
    starting.value = false;
    cancelPendingStart.value = false;
    await clearSensitiveStartState();
  }
}

async function cancelRun(): Promise<void> {
  if (starting.value && !runtime.activeRunId.value) {
    cancelPendingStart.value = true;
    ElMessage.info("启动完成后将立即终止打包");
    return;
  }
  try {
    await runtime.cancel();
    cancelPendingStart.value = false;
    ElMessage.info("已请求终止打包");
  } catch (error) {
    showError(error);
  }
}

function validateArchiveFolderName(value: string): string | null {
  if (!value.trim()) return "请输入归档目录名";
  if (value !== value.trim()) return "归档目录名首尾不能包含空格";
  if (value === "." || value === "..") return "归档目录名不能为 . 或 ..";
  if (value.length > 255) return "归档目录名不能超过 255 个字符";
  if (/[<>:\"/\\|?*\u0000-\u001f]/.test(value)) return "归档目录名包含 Windows 非法字符";
  if (/[. ]$/.test(value)) return "归档目录名不能以点或空格结尾";
  if (/^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$/i.test(value)) {
    return "归档目录名不能使用 Windows 保留设备名";
  }
  return null;
}

function formatUploadBytes(value: number): string {
  if (value < 1_024) return `${value} B`;
  if (value < 1_048_576) return `${(value / 1_024).toFixed(1)} KB`;
  if (value < 1_073_741_824) return `${(value / 1_048_576).toFixed(1)} MB`;
  return `${(value / 1_073_741_824).toFixed(1)} GB`;
}

async function openArchive(): Promise<void> {
  if (!archivePath.value) return;
  try {
    await invokeToolByChannel("tool:system:open-local-path", { path: archivePath.value });
  } catch (error) {
    showError(error);
  }
}

async function scrollLogToBottom(container: HTMLElement | null): Promise<void> {
  await nextTick();
  if (!container) return;
  const nearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 48;
  if (nearBottom) container.scrollTop = container.scrollHeight;
}

watch(() => frontendLogs.value.length, () => scrollLogToBottom(frontendLogContainer.value));
watch(() => backendLogs.value.length, () => scrollLogToBottom(backendLogContainer.value));
watch(() => uploadLogs.value.length, () => scrollLogToBottom(uploadLogContainer.value));
watch(() => draft.packageType, (packageType) => {
  serverConfigSections.value = packageType === "server_upload" ? ["server"] : [];
});
watch(selectedId, () => {
  titleEditing.value = false;
});
watchPendingIntent("release-package", applyActionDispatchIntent);

onMounted(async () => {
  void loadVaultServerOptions();
  try {
    await runtime.ensureListeners();
    await loadProjects();
  } catch (error) {
    showError(error);
  }
});
</script>

<style scoped>
.release-package-panel {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: 14px;
  width: 100%;
  min-height: 100%;
}
.release-package-workspace {
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  min-height: 0;
  overflow: visible;
  border: 1px solid #e4e7ed;
  border-radius: 10px;
  background: #f7f8fa;
  box-shadow: 0 4px 18px rgb(31 45 61 / 5%);
}
.release-package-projects {
  padding: 14px 12px;
  border-right: 1px solid #e4e7ed;
  background: #fbfcfd;
}
.projects-heading, .projects-actions, .editor-header, .editor-actions, .engineering-card-header, .command-label-row, .log-card-header, .command-example-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.projects-heading { margin-bottom: 8px; color: #303133; }
.projects-empty, .log-empty { color: var(--lc-text-secondary, #909399); font-size: 13px; }
.project-item {
  display: flex;
  width: 100%;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  margin-top: 4px;
  padding: 9px 10px;
  border: 1px solid transparent;
  border-radius: 7px;
  color: inherit;
  background: transparent;
  cursor: pointer;
  text-align: left;
}
.project-item:hover { border-color: #dcdfe6; background: #fff; }
.project-item.active { border-color: #b9d7fb; color: var(--el-color-primary, #409eff); background: #eef6ff; }
.project-item:disabled { cursor: not-allowed; opacity: .65; }
.project-name { overflow: hidden; max-width: 100%; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.project-updated { color: var(--lc-text-secondary, #909399); font-size: 11px; }
.release-package-editor { min-width: 0; padding: 18px; }
.editor-header {
  align-items: flex-start;
  padding: 16px;
  border-bottom: 1px solid #ebeef5;
}
.editor-title { min-width: 0; flex: 1; }
.editor-header h2 { margin: 0; color: #303133; font-size: 18px; }
.project-title {
  overflow: hidden;
  max-width: 100%;
  padding: 0;
  border: 0;
  color: inherit;
  background: transparent;
  cursor: text;
  font: inherit;
  font-weight: 600;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.project-title:disabled { cursor: default; }
.project-title:focus-visible {
  border-radius: 3px;
  outline: 2px solid var(--el-color-primary, #409eff);
  outline-offset: 3px;
}
.project-title-input { width: min(360px, 100%); }
.project-title-input :deep(.el-input__inner) { font-size: 18px; font-weight: 600; }
.editor-actions { flex-wrap: wrap; justify-content: flex-end; }
.release-package-form { min-width: 0; }
.release-package-form :deep(.el-form-item) { margin-bottom: 14px; }
.project-overview, .engineering-card {
  border: 1px solid #e4e7ed;
  border-radius: 9px;
  background: #fff;
  box-shadow: 0 2px 10px rgb(31 45 61 / 4%);
}
.project-overview { margin-bottom: 14px; overflow: hidden; }
.project-basics { padding: 14px 16px 0; }
.project-basics-grid {
  display: grid;
  grid-template-columns: minmax(240px, 320px) minmax(0, 1fr);
  gap: 14px;
}
.engineering-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
  gap: 14px;
  align-items: start;
}
.engineering-card { min-width: 0; padding: 16px 16px 2px; }
.engineering-card-header { align-items: flex-start; margin-bottom: 16px; padding-bottom: 11px; border-bottom: 1px solid #ebeef5; }
.engineering-card-header h3 { margin: 2px 0 0; color: #303133; font-size: 16px; }
.engineering-kicker { color: var(--el-color-primary, #409eff); font-size: 10px; font-weight: 700; letter-spacing: .12em; }
.engineering-index { color: #c0c4cc; font: 600 20px/1 var(--lc-font-mono, Consolas, monospace); }
.command-label-row { width: 100%; }
.command-label-row :deep(.el-button) { height: auto; min-height: 22px; padding: 2px 4px; }
.command-input { width: 100%; }
.command-input :deep(.el-textarea__inner) {
  resize: vertical;
  font-family: var(--lc-font-mono, Consolas, monospace);
  line-height: 1.55;
}
.command-hint { margin: 7px 0 0; color: #909399; font-size: 12px; line-height: 1.55; }
.artifact-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(150px, .65fr); gap: 12px; }
.full-width { width: 100%; }
.server-config-collapse {
  margin-top: 14px;
  border: 1px solid #e4e7ed;
  border-radius: 9px;
  background: #fff;
  box-shadow: 0 2px 10px rgb(31 45 61 / 4%);
}
.server-config-collapse :deep(.el-collapse-item__header) {
  height: auto;
  min-height: 54px;
  padding: 0 16px;
  border-bottom: 0;
  border-radius: 9px;
}
.server-config-collapse :deep(.el-collapse-item__wrap) { border-bottom: 0; }
.server-config-collapse :deep(.el-collapse-item__content) { padding: 0; }
.server-config-heading {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  padding-right: 10px;
  color: #303133;
}
.server-config-heading > div { display: grid; min-width: 0; line-height: 1.45; }
.server-config-heading span { color: #606266; font-size: 12px; font-weight: 400; }
.server-config-body {
  display: grid;
  gap: 18px;
  padding: 0 16px 2px;
  border-top: 1px solid #ebeef5;
}
.server-config-section { display: grid; gap: 10px; min-width: 0; }
.server-config-section-heading {
  display: grid;
  gap: 2px;
  padding-top: 2px;
}
.server-config-section-heading strong { color: #303133; font-size: 13px; }
.server-config-section-heading span { color: #606266; font-size: 12px; line-height: 1.45; }
.server-auth-type-row { width: min(320px, 100%); }
.server-auth-details,
.server-auth-details-panel { min-width: 0; }
.private-key-config-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0 12px;
}
.private-key-file-field { grid-column: 1 / -1; }
.server-target-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0 12px;
}
.server-command-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0 12px;
}
.server-command-grid :deep(.el-textarea__inner) {
  resize: vertical;
  font-family: var(--lc-font-mono, Consolas, monospace);
  line-height: 1.55;
}
.server-command-note { margin: -4px 0 12px; color: #606266; font-size: 12px; line-height: 1.5; }
.vault-credential-field :deep(.el-form-item__content) { display: grid; gap: 8px; }
.vault-credential-picker { display: flex; align-items: center; gap: 8px; width: 100%; min-width: 0; }
.vault-credential-picker :deep(.el-select) { min-width: 180px; }
.vault-credential-hint { margin: 0; color: #606266; font-size: 12px; line-height: 1.5; }
.vault-binding-invalid {
  padding: 8px 10px;
  border: 1px solid #f3c4c4;
  border-radius: 6px;
  color: #b42318;
  background: #fff5f5;
  font-size: 12px;
}
.vault-credential-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.vault-credential-summary > div {
  display: grid;
  min-width: 0;
  gap: 3px;
  padding: 8px 10px;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  background: #fafbfc;
}
.vault-credential-summary span { color: #909399; font-size: 11px; }
.vault-credential-summary code { overflow-wrap: anywhere; color: #303133; font: 12px/1.4 var(--lc-font-mono, Consolas, monospace); }
.auth-type-group, .package-type-group { display: flex; width: 100%; }
.auth-type-group :deep(.el-radio-button), .package-type-group :deep(.el-radio-button) { flex: 1; }
.auth-type-group :deep(.el-radio-button__inner), .package-type-group :deep(.el-radio-button__inner) { width: 100%; }
.release-package-log-card {
  overflow: hidden;
  border: 1px solid #e4e7ed;
  border-radius: 10px;
  background: #fff;
  box-shadow: 0 2px 12px rgb(31 45 61 / 5%);
}
.release-package-project-log { margin-top: 14px; }
.log-card-header { flex-wrap: wrap; padding: 12px 14px; border-bottom: 1px solid #ebeef5; }
.log-card-header h3 { margin: 0 0 3px; color: #303133; font-size: 15px; }
.log-card-header p { margin: 0; color: #5f6b7a; font-size: 12px; }
.log-status { flex: none; }
.release-package-log-card :deep(.log-status.el-tag--primary) {
  --el-tag-text-color: #1d4ed8;
  --el-tag-bg-color: #eff6ff;
  --el-tag-border-color: #bfdbfe;
}
.release-package-log-card :deep(.log-status.el-tag--success) {
  --el-tag-text-color: #237a3b;
  --el-tag-bg-color: #eefbf2;
  --el-tag-border-color: #b7e4c3;
}
.release-package-log-card :deep(.log-status.el-tag--info) {
  --el-tag-text-color: #4b5563;
  --el-tag-bg-color: #f3f4f6;
  --el-tag-border-color: #d1d5db;
}
.release-package-log-card :deep(.log-status.el-tag--warning) {
  --el-tag-text-color: #8a4b08;
  --el-tag-bg-color: #fff7ed;
  --el-tag-border-color: #fed7aa;
}
.release-package-log-card :deep(.log-status.el-tag--danger) {
  --el-tag-text-color: #b42318;
  --el-tag-bg-color: #fff1f0;
  --el-tag-border-color: #fecaca;
}
.log-error-summary {
  min-width: 0;
  padding: 8px 10px;
  border-left: 3px solid #dc2626;
  overflow-wrap: anywhere;
  color: #b42318;
  background: #fff5f5;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.log-error-summary.warning { border-left-color: #d97706; color: #8a4b08; background: #fff7ed; }
.log-overall-error { flex: 1 0 100%; margin-top: 3px; }
.log-lane-error { border-bottom: 1px solid #f3d1d1; }
.log-lane-error.warning { border-bottom-color: #fed7aa; }
.release-package-log-columns { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.release-package-log-columns.has-upload-lane { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.release-package-log-lane + .release-package-log-lane { border-left: 1px solid #ebeef5; }
.log-lane-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 9px 14px;
  border-bottom: 1px solid #ebeef5;
  color: #303133;
  background: #fafbfc;
}
.log-lane-actions { display: inline-flex; align-items: center; gap: 4px; }
.upload-lane-header { align-items: flex-start; flex-wrap: wrap; }
.upload-lane-title { display: grid; min-width: 0; gap: 2px; }
.upload-lane-actions { display: flex; flex: 1 1 220px; flex-wrap: wrap; align-items: center; justify-content: flex-end; gap: 6px; }
.command-status-tags { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 4px; }
.upload-current-path {
  overflow: hidden;
  max-width: 100%;
  color: #606266;
  font: 11px/1.4 var(--lc-font-mono, Consolas, monospace);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.upload-progress {
  display: grid;
  gap: 5px;
  padding: 9px 14px;
  border-bottom: 1px solid #ebeef5;
  color: #606266;
  background: #fbfcfd;
  font-size: 11px;
}
.upload-progress :deep(.el-progress-bar) { margin-right: 0; padding-right: 0; }
.upload-progress :deep(.el-progress__text) { display: none; }
.release-package-log {
  min-height: 180px;
  max-height: 320px;
  overflow: auto;
  padding: 12px 14px;
  color: #303133;
  background: #fff;
  font: 12px/1.65 var(--lc-font-mono, Consolas, monospace);
}
.log-line { display: flex; gap: 8px; white-space: pre-wrap; word-break: break-word; }
.log-line.stderr { color: #d03050; }
.log-meta { flex: none; color: #5f6b7a; }
.archive-preview { margin: 0; overflow-wrap: anywhere; color: var(--lc-text-secondary, #606266); font-size: 13px; }
.vault-start-summary {
  display: grid;
  gap: 4px;
  padding: 11px 12px;
  border: 1px solid #d9e8fb;
  border-radius: 8px;
  color: #303133;
  background: #f5f9ff;
}
.vault-start-summary span { color: #606266; font-size: 12px; }
.vault-start-summary p { margin: 0; color: #606266; font-size: 12px; line-height: 1.5; }
.command-retry-notice {
  margin: 0 0 14px;
  padding: 10px 12px;
  border: 1px solid #fed7aa;
  border-radius: 7px;
  color: #8a4b08;
  background: #fff7ed;
  font-size: 13px;
  line-height: 1.55;
}
.command-retry-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-bottom: 14px; }
.command-retry-summary > div {
  display: grid;
  min-width: 0;
  gap: 3px;
  padding: 8px 10px;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  background: #fafbfc;
}
.command-retry-summary span { color: #909399; font-size: 11px; }
.command-retry-summary code { overflow-wrap: anywhere; color: #303133; font: 12px/1.4 var(--lc-font-mono, Consolas, monospace); }
.command-retry-summary strong { color: #303133; font-size: 12px; }
.package-targets { display: grid; gap: 8px; margin-top: 16px; }
.package-targets-label { color: #303133; font-size: 14px; font-weight: 600; }
.package-targets :deep(.el-checkbox-group) { display: flex; flex-wrap: wrap; gap: 10px; }
.package-targets :deep(.el-checkbox) {
  flex: 1 1 180px;
  height: auto;
  margin: 0;
  padding: 10px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #fff;
}
.package-targets :deep(.el-checkbox.is-checked) { border-color: #a8c7fa; background: #f5f9ff; }
.preflight-summary { display: grid; gap: 8px; margin-top: 16px; }
.preflight-host, .preflight-target-row {
  display: grid;
  grid-template-columns: 80px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 9px 10px;
  border: 1px solid #e4e7ed;
  border-radius: 7px;
  color: #606266;
  background: #fafbfc;
  font-size: 12px;
}
.preflight-host { grid-template-columns: 80px minmax(0, 1fr); }
.preflight-host code, .preflight-target-row code { overflow-wrap: anywhere; color: #303133; }
.preflight-targets { display: grid; gap: 6px; }

:global(.release-package-command-examples) {
  max-width: calc(100vw - 32px);
  padding: 10px !important;
  border-color: #dcdfe6 !important;
  background: #fff !important;
  box-shadow: 0 10px 30px rgb(31 45 61 / 14%) !important;
}
:global(.release-package-command-examples .command-example-list) {
  display: grid;
  gap: 8px;
  max-height: min(560px, calc(100vh - 120px));
  overflow: auto;
}
:global(.release-package-command-examples .command-example-item) {
  padding: 10px;
  border: 1px solid #e4e7ed;
  border-radius: 7px;
  color: #303133;
  background: #fff;
}
:global(.release-package-command-examples .command-example-heading) { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
:global(.release-package-command-examples .command-example-heading strong) { font-size: 13px; }
:global(.release-package-command-examples .command-example-item p) { margin: 5px 0 8px; color: #606266; font-size: 12px; line-height: 1.5; }
:global(.release-package-command-examples .command-example-item pre) {
  overflow-x: auto;
  margin: 0;
  padding: 9px 10px;
  border: 1px solid #ebeef5;
  border-radius: 5px;
  color: #303133;
  background: #f7f8fa;
  font: 11px/1.55 var(--lc-font-mono, Consolas, monospace);
  white-space: pre-wrap;
  word-break: break-word;
}
:global(.release-package-host-trust), :global(.release-package-remote-overwrite) {
  display: grid;
  gap: 8px;
  color: #303133;
  font-size: 13px;
}
:global(.release-package-host-trust .host-trust-row) {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 8px;
}
:global(.release-package-host-trust code), :global(.release-package-remote-overwrite code) {
  overflow-wrap: anywhere;
  color: #303133;
  font: 12px/1.5 var(--lc-font-mono, Consolas, monospace);
}
:global(.release-package-remote-overwrite p) { margin: 0; }
:global(.release-package-remote-overwrite ul) { display: grid; gap: 6px; margin: 0; padding-left: 18px; }
@media (max-width: 960px) {
  .release-package-workspace { grid-template-columns: 1fr; }
  .release-package-projects { display: flex; gap: 8px; overflow-x: auto; border-right: 0; border-bottom: 1px solid #e4e7ed; }
  .projects-heading { flex: none; flex-direction: column; align-items: flex-start; }
  .project-item { flex: 0 0 150px; }
  .release-package-editor { padding: 14px; }
  .private-key-config-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .server-target-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .vault-credential-field { grid-column: 1 / -1; }
  .release-package-log-columns.has-upload-lane { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .upload-log-lane { grid-column: 1 / -1; border-top: 1px solid #ebeef5; border-left: 0 !important; }
}
@media (max-width: 640px) {
  .editor-header { flex-direction: column; }
  .editor-actions { justify-content: flex-start; }
  .artifact-grid { grid-template-columns: 1fr; gap: 0; }
  .project-basics-grid { grid-template-columns: 1fr; gap: 0; }
  .server-auth-type-row { width: 100%; }
  .server-config-heading { align-items: flex-start; }
  .private-key-config-grid { grid-template-columns: 1fr; }
  .server-target-grid { grid-template-columns: 1fr; }
  .server-command-grid { grid-template-columns: 1fr; }
  .command-retry-summary { grid-template-columns: 1fr; }
  .private-key-file-field { grid-column: auto; }
  .vault-credential-field { grid-column: auto; }
  .vault-credential-picker { flex-wrap: wrap; }
  .vault-credential-picker :deep(.el-select) { flex: 1 1 100%; }
  .vault-credential-summary { grid-template-columns: 1fr; }
  .release-package-editor { padding: 10px; }
  .engineering-card { padding: 14px 12px 0; }
  .log-card-header { align-items: flex-start; }
  .upload-lane-actions, .command-status-tags { justify-content: flex-start; }
  .release-package-log-columns { grid-template-columns: 1fr; }
  .upload-log-lane { grid-column: auto; }
  .release-package-log-lane + .release-package-log-lane { border-top: 1px solid #ebeef5; border-left: 0; }
  .preflight-target-row { grid-template-columns: 58px minmax(0, 1fr); }
  .preflight-target-row :deep(.el-tag) { grid-column: 2; justify-self: start; }
}
</style>
