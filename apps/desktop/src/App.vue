<template>
  <div class="shell">
    <aside class="nav">
      <el-text tag="h2" size="large">Lazycat 懒猫</el-text>
      <el-menu :default-active="activeTool" @select="onSelect">
        <el-menu-item index="home">首页</el-menu-item>
        <el-sub-menu v-for="group in groups" :key="group.id" :index="group.id">
          <template #title>{{ group.name }}</template>
          <el-menu-item v-for="tool in group.tools" :key="tool.id" :index="tool.id">
            {{ tool.name }}
          </el-menu-item>
        </el-sub-menu>
        <el-menu-item index="settings">设置</el-menu-item>
      </el-menu>
    </aside>

    <main ref="contentRef" class="content">
      <div class="tool-header">
        <h1 class="tool-title">{{ currentTool?.name }}</h1>
        <el-button
          v-if="activeTool !== HOME_ID && isRealToolId(activeTool)"
          text
          :type="isFavorite(activeTool) ? 'warning' : 'info'"
          @click="toggleFavorite(activeTool)"
        >
          {{ isFavorite(activeTool) ? "取消收藏" : "收藏" }}
        </el-button>
      </div>
      <p class="tool-desc">{{ currentTool?.desc }}</p>

      <HomePanel
        v-if="activeTool === HOME_ID"
        :favorite-tools="favoriteTools"
        :top-monthly-tools="topMonthlyTools"
        :home-top-limit="homeTopLimit"
        :is-favorite="isFavorite"
        @open-tool="openToolFromHome"
        @toggle-favorite="toggleFavorite"
        @update:home-top-limit="homeTopLimit = $event"
      />

      <EncodePanel
        v-else-if="['base64', 'url', 'md5', 'qr'].includes(activeTool)"
        :active-tool="activeTool"
      />

      <CalcDraftPanel
        v-else-if="activeTool === 'calc-draft'"
        :history="calcHistory"
        :current-input="calcCurrentInput"
        :current-preview="calcCurrentPreview"
        :format-history-time="formatCalcHistoryTime"
        @update:current-input="onCalcInput"
        @enter="onCalcEnter"
        @clear-history="clearCalcHistory"
        @history-click="onCalcHistoryClick"
      />

      <div v-else-if="activeTool === 'rsa'" class="panel-grid">
        <el-input v-model="cryptoInput" type="textarea" :rows="8" placeholder="明文 / 密文(Base64)" />
        <el-input v-model="cryptoOutput" type="textarea" :rows="8" readonly placeholder="输出" />
        <el-input v-model="publicKeyPem" class="panel-grid-full" type="textarea" :rows="6" placeholder="RSA 公钥 PEM" />
        <el-input v-model="privateKeyPem" class="panel-grid-full" type="textarea" :rows="6" placeholder="RSA 私钥 PEM" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="rsaEncryptAction">RSA 加密</el-button>
            <el-button @click="rsaDecryptAction">RSA 解密</el-button>
          </el-space>
        </div>
      </div>

      <div v-else-if="activeTool === 'aes'" class="panel-grid">
        <el-input v-model="cryptoInput" type="textarea" :rows="6" placeholder="明文 / 密文(Base64)" />
        <el-input v-model="cryptoOutput" type="textarea" :rows="6" readonly placeholder="输出" />
        <el-select v-model="symmetricAlgorithm" placeholder="算法">
          <el-option label="AES-256-CBC" value="aes-256-cbc" />
          <el-option label="AES-192-CBC" value="aes-192-cbc" />
          <el-option label="AES-128-CBC" value="aes-128-cbc" />
          <el-option label="3DES-CBC" value="des-ede3-cbc" />
          <el-option label="DES-CBC" value="des-cbc" />
        </el-select>
        <el-input v-model="symmetricIv" placeholder="IV（文本）" />
        <el-input class="panel-grid-full" v-model="symmetricKey" placeholder="Key（文本）" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="symmetricEncrypt">加密</el-button>
            <el-button @click="symmetricDecrypt">解密</el-button>
          </el-space>
        </div>
      </div>

      <FormatterPanel
        v-else-if="activeTool === 'formatter'"
        :input="formatInput"
        :output="formatOutput"
        :language="monacoLanguage"
        :detected-label="formatDetectedLabel"
        @update:input="formatInput = $event"
        @format="formatCode"
      />

      <div v-else-if="activeTool === 'json-xml'" class="panel-grid">
        <el-input v-model="convertInput" type="textarea" :rows="12" placeholder="输入 JSON 或 XML" />
        <el-input v-model="convertOutput" type="textarea" :rows="12" readonly placeholder="转换结果" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="runConvertTool('tool:convert:json-to-xml')">JSON -> XML</el-button>
            <el-button @click="runConvertTool('tool:convert:xml-to-json')">XML -> JSON</el-button>
          </el-space>
        </div>
      </div>

      <div v-else-if="activeTool === 'json-yaml'" class="panel-grid">
        <el-input v-model="convertInput" type="textarea" :rows="12" placeholder="输入 JSON" />
        <el-input v-model="convertOutput" type="textarea" :rows="12" readonly placeholder="YAML 结果" />
        <div class="panel-grid-full">
          <el-button type="primary" @click="runConvertTool('tool:convert:json-to-yaml')">JSON -> YAML</el-button>
        </div>
      </div>

      <div v-else-if="activeTool === 'csv-json'" class="panel-grid">
        <el-input v-model="convertInput" type="textarea" :rows="12" placeholder="输入 CSV" />
        <el-input v-model="convertOutput" type="textarea" :rows="12" readonly placeholder="JSON 结果" />
        <el-input v-model="csvDelimiter" placeholder="分隔符，默认逗号" />
        <div>
          <el-button type="primary" @click="csvToJsonAction">CSV -> JSON</el-button>
        </div>
      </div>

      <div v-else-if="activeTool === 'text-process'" class="panel-grid">
        <el-input v-model="textProcessInput" type="textarea" :rows="12" placeholder="输入多行文本" />
        <el-input v-model="textProcessOutput" type="textarea" :rows="12" readonly placeholder="处理结果" />
        <el-switch v-model="textCaseSensitive" active-text="区分大小写" />
        <div>
          <el-space>
            <el-button type="primary" @click="dedupeLines">按行去重</el-button>
            <el-button @click="sortTextLines">按行排序</el-button>
          </el-space>
        </div>
      </div>

      <RegexPanel
        v-else-if="activeTool === 'regex'"
        :pattern="regexPattern"
        :flags="regexFlags"
        :input="regexInput"
        :output="regexOutput"
        :kind="regexKind"
        :templates="regexTemplates"
        @update:pattern="regexPattern = $event"
        @update:flags="regexFlags = $event"
        @update:input="regexInput = $event"
        @update:kind="regexKind = $event"
        @run="runRegexTest"
        @apply-template="applyRegexTemplate"
        @load-templates="loadRegexTemplates"
      />

      <NetworkPanel v-else-if="activeTool === 'network'" />

      <HostsPanel
        v-else-if="activeTool === 'hosts'"
        :name="hostsName"
        :content="hostsContent"
        :profiles="hostsProfiles"
        @update:name="hostsName = $event"
        @update:content="hostsContent = $event"
        @save="saveHosts"
        @activate="activateHosts"
        @delete="deleteHosts"
        @refresh="loadHostsProfiles"
        @pick="pickHosts"
      />

      <PortsPanel
        v-else-if="activeTool === 'ports'"
        :summary="portUsageSummary"
        :state-rows="portUsageStateRows"
        :filter="portFilter"
        :filtered-process-rows="filteredPortProcessRows"
        :connection-rows="portConnectionRows"
        @load="loadPortUsage"
        @update:filter="portFilter = $event"
      />

      <div v-else-if="activeTool === 'env'" class="panel-grid">
        <div class="panel-grid-full">
          <el-button type="primary" @click="detectEnv">检测 Node / Java 版本</el-button>
        </div>
        <el-input class="panel-grid-full" v-model="envOutput" type="textarea" :rows="8" readonly />
      </div>

      <div v-else-if="activeTool === 'split-merge'" class="panel-grid">
        <el-input v-model="sourcePath" placeholder="源文件路径" />
        <el-input v-model="outputDir" placeholder="分片输出目录" />
        <el-input-number v-model="chunkSizeMb" :min="1" :max="2048" />
        <div>
          <el-button type="primary" @click="splitFile">切割文件</el-button>
        </div>
        <el-input class="panel-grid-full" v-model="partsInput" type="textarea" :rows="5" placeholder="待合并分片路径（每行一个）" />
        <el-input v-model="mergeOutputPath" placeholder="合并输出文件路径" />
        <div>
          <el-button @click="mergeFiles">合并文件</el-button>
        </div>
        <el-input class="panel-grid-full" v-model="fileToolOutput" type="textarea" :rows="10" readonly />
      </div>

      <div v-else-if="activeTool === 'image'" class="panel-grid">
        <el-input v-model="imageInputPath" placeholder="输入图片路径" />
        <el-input v-model="imageOutputPath" placeholder="输出图片路径（含后缀）" />
        <el-select v-model="imageFormat">
          <el-option label="png" value="png" />
          <el-option label="jpeg" value="jpeg" />
          <el-option label="webp" value="webp" />
          <el-option label="avif" value="avif" />
        </el-select>
        <el-input-number v-model="imageWidth" :min="1" :max="10000" />
        <el-input-number v-model="imageHeight" :min="1" :max="10000" />
        <el-input-number v-model="cropX" :min="0" :max="10000" />
        <el-input-number v-model="cropY" :min="0" :max="10000" />
        <el-input-number v-model="cropWidth" :min="1" :max="10000" />
        <el-input-number v-model="cropHeight" :min="1" :max="10000" />
        <el-input-number v-model="imageQuality" :min="1" :max="100" />
        <div class="panel-grid-full">
          <el-button type="primary" @click="convertImageAction">转换图片</el-button>
        </div>
        <el-input class="panel-grid-full" v-model="imageOutput" type="textarea" :rows="8" readonly />
      </div>

      <div v-else-if="activeTool === 'timestamp'" class="panel-grid">
        <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
          <span style="white-space:nowrap;color:var(--el-text-color-secondary);font-size:13px;">时间戳 → 日期</span>
          <el-input v-model="timeInput" placeholder="时间戳" style="flex:1;" />
          <el-button-group>
            <el-button :type="timePrecision === 's' ? 'primary' : ''" @click="timePrecision = 's'; onTimePrecisionChange()">秒</el-button>
            <el-button :type="timePrecision === 'ms' ? 'primary' : ''" @click="timePrecision = 'ms'; onTimePrecisionChange()">毫秒</el-button>
          </el-button-group>
          <el-input v-model="timeOutput" readonly placeholder="日期结果" style="flex:1;" />
        </div>
        <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
          <span style="white-space:nowrap;color:var(--el-text-color-secondary);font-size:13px;">日期 → 时间戳</span>
          <el-input v-model="dateInput" placeholder="日期，如 2024-01-01 00:00:00" style="flex:1;" />
          <el-button-group>
            <el-button :type="datePrecision === 's' ? 'primary' : ''" @click="datePrecision = 's'; onDatePrecisionChange()">秒</el-button>
            <el-button :type="datePrecision === 'ms' ? 'primary' : ''" @click="datePrecision = 'ms'; onDatePrecisionChange()">毫秒</el-button>
          </el-button-group>
          <el-input v-model="dateOutput" readonly placeholder="时间戳结果" style="flex:1;" />
        </div>
      </div>

      <div v-else-if="activeTool === 'uuid'" class="panel-grid">
        <el-input-number v-model="passwordLength" :min="4" :max="128" />
        <el-switch v-model="passwordSymbols" active-text="含符号" />
        <el-switch v-model="passwordNumbers" active-text="含数字" />
        <el-switch v-model="passwordUppercase" active-text="含大写" />
        <el-switch v-model="passwordLowercase" active-text="含小写" />
        <el-input class="panel-grid-full" v-model="idOutput" type="textarea" :rows="8" readonly />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="generateUuid">生成 UUID</el-button>
            <el-button @click="generateGuidAction">生成 GUID</el-button>
            <el-button @click="generatePasswordAction">生成密码</el-button>
          </el-space>
        </div>
      </div>

      <div v-else-if="activeTool === 'cron'" class="panel-grid">
        <div class="panel-grid-full" style="display:flex;flex-direction:column;gap:8px;width:50%;">
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">秒</span>
            <el-input v-model="cronSecond" placeholder="0" style="flex:1;" />
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">分</span>
            <el-input v-model="cronMinute" placeholder="*" style="flex:1;" />
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">时</span>
            <el-input v-model="cronHour" placeholder="*" style="flex:1;" />
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">日</span>
            <el-input v-model="cronDom" placeholder="*" style="flex:1;" />
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">月</span>
            <el-input v-model="cronMonth" placeholder="*" style="flex:1;" />
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">周</span>
            <el-input v-model="cronDow" placeholder="*" style="flex:1;" />
          </div>
        </div>
        <el-input class="panel-grid-full" v-model="cronExpression" placeholder="Cron 表达式（可直接粘贴后点解析）" style="margin-left:36px;width:calc(50% - 36px);" />
        <div class="panel-grid-full">
          <el-space>
            <el-button @click="parseCron">解析表达式</el-button>
            <el-button @click="previewCron">预览触发时间</el-button>
          </el-space>
        </div>
        <el-input class="panel-grid-full" v-model="cronOutput" type="textarea" :rows="8" readonly />
      </div>

      <ManualPanel v-else-if="activeTool === 'manuals'" />

      <div v-else-if="activeTool === 'settings'" class="panel-grid">
        <div class="panel-grid-full">
          <p style="margin-bottom: 8px; color: var(--el-text-color-secondary); font-size: 13px;">
            设置全局快捷键后，可在任意位置显示/隐藏主窗口。关闭窗口时会最小化到系统托盘。
          </p>
          <el-form label-width="120px" style="max-width: 480px;">
            <el-form-item label="显示/隐藏快捷键">
              <el-input
                v-model="hotkeyInput"
                placeholder="例如：Alt+Space 或 Ctrl+Shift+L"
                clearable
                style="width: 260px;"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="saveHotkeySettings">保存</el-button>
              <el-button @click="clearHotkeySettings" style="margin-left: 8px;">清除快捷键</el-button>
            </el-form-item>
          </el-form>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import HomePanel from "./components/HomePanel.vue";
import CalcDraftPanel from "./components/CalcDraftPanel.vue";
import FormatterPanel from "./components/FormatterPanel.vue";
import RegexPanel from "./components/RegexPanel.vue";
import HostsPanel from "./components/HostsPanel.vue";
import PortsPanel from "./components/PortsPanel.vue";
import NetworkPanel from "./components/NetworkPanel.vue";
import ManualPanel from "./components/ManualPanel.vue";
import EncodePanel from "./components/EncodePanel.vue";
import { invokeToolByChannel, registerHotkey, unregisterHotkey } from "./bridge/tauri";
import { formatHtml, formatJava, formatJson, formatSqlCode, formatXml } from "@lazycat/formatters";

interface ToolDef {
  id: string;
  name: string;
  desc: string;
}
interface GroupDef {
  id: string;
  name: string;
  tools: ToolDef[];
}
interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean;
  updatedAt: string;
}
interface PortUsageSummary {
  total: number;
  tcp: number;
  udp: number;
}
interface PortUsageStateRow {
  state: string;
  count: number;
}
interface PortUsageProcessRow {
  pid: number;
  processName: string;
  listeningPorts: string[];
  listeningPortsText: string;
  connectionCount: number;
}
interface PortUsageConnectionRow {
  protocol: string;
  pid: number;
  processName: string;
  localAddress: string;
  remoteAddress: string;
  state: string;
}
interface CalcDraftEntry {
  id: number;
  expression: string;
  resultRaw: string;
  resultDisplay: string;
  createdAt: number;
}
type ToolClickHistory = Record<string, number[]>;

const groups: GroupDef[] = [
  {
    id: "codec",
    name: "编码与加密",
    tools: [
      { id: "base64", name: "Base64", desc: "Base64 编码与解码" },
      { id: "url", name: "URL 编解码", desc: "URL Encode / Decode" },
      { id: "md5", name: "MD5", desc: "计算 MD5 摘要" },
      { id: "qr", name: "二维码生成", desc: "根据文本生成二维码" },
      { id: "rsa", name: "RSA 加解密", desc: "RSA 公私钥加解密" },
      { id: "aes", name: "AES/DES", desc: "AES 与 DES/3DES 加解密" }
    ]
  },
  {
    id: "format",
    name: "格式化与转换",
    tools: [
      { id: "formatter", name: "代码格式化", desc: "JSON/XML/HTML/Java/SQL 自动识别" },
      { id: "json-xml", name: "JSON/XML", desc: "JSON 与 XML 双向转换" },
      { id: "json-yaml", name: "JSON/YAML", desc: "JSON 转 YAML" },
      { id: "csv-json", name: "CSV/JSON", desc: "CSV 转 JSON" }
    ]
  },
  {
    id: "text",
    name: "文本与正则",
    tools: [
      { id: "text-process", name: "文本处理", desc: "按行去重与排序" },
      { id: "regex", name: "正则工具", desc: "表达式生成与测试" }
    ]
  },
  {
    id: "system",
    name: "系统与网络",
    tools: [
      { id: "network", name: "IP/端口连通", desc: "TCP 连通性测试" },
      { id: "hosts", name: "Hosts 管理", desc: "多配置保存与切换" },
      { id: "ports", name: "端口占用", desc: "端口与进程查询" },
      { id: "env", name: "环境检测", desc: "Node 与 Java 版本检测" }
    ]
  },
  {
    id: "files",
    name: "文件与图片",
    tools: [
      { id: "split-merge", name: "切割与合并", desc: "大文件切片与合并" },
      { id: "image", name: "图片转换", desc: "格式转换、缩放、裁剪、压缩" }
    ]
  },
  {
    id: "misc",
    name: "时间与生成器",
    tools: [
      { id: "calc-draft", name: "计算草稿", desc: "草稿式计算，回车复制结果并保留历史" },
      { id: "timestamp", name: "时间戳转换", desc: "时间戳与日期互转" },
      { id: "uuid", name: "UUID/GUID/密码", desc: "标识与随机密码生成" },
      { id: "cron", name: "Cron 工具", desc: "Cron 表达式生成与预览" },
      { id: "manuals", name: "离线手册", desc: "Vue3 开发手册" }
    ]
  }
];

const HOME_ID = "home";
const HOME_TOOL: ToolDef = {
  id: HOME_ID,
  name: "首页",
  desc: "收藏页面与最近一个月高频功能入口"
};
const FAVORITE_STORAGE_KEY = "lazycat:favorites:v1";
const TOOL_CLICKS_STORAGE_KEY = "lazycat:tool-clicks:v1";
const HOME_TOP_LIMIT_STORAGE_KEY = "lazycat:home-top-limit:v1";
const HOTKEY_STORAGE_KEY = "lazycat:hotkey:v1";
const CALC_DRAFT_HISTORY_STORAGE_KEY = "lazycat:calc-draft-history:v1";
const CLICK_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;
const MAX_CLICK_HISTORY_PER_TOOL = 500;
const MAX_CALC_HISTORY = 200;

const allTools = groups.flatMap((g) => g.tools);
const allToolMap = new Map(allTools.map((tool) => [tool.id, tool]));

const contentRef = ref<HTMLElement | null>(null);
const activeTool = ref(HOME_ID);
const favoriteToolIds = ref<string[]>([]);
const toolClickHistory = ref<ToolClickHistory>({});
const homeTopLimit = ref<6 | 12>(12);
const hotkeyInput = ref("");

const currentTool = computed(() => {
  if (activeTool.value === HOME_ID) {
    return HOME_TOOL;
  }
  if (activeTool.value === "settings") {
    return { id: "settings", name: "设置", desc: "快捷键与应用偏好设置" };
  }
  return allToolMap.get(activeTool.value);
});
const favoriteTools = computed(() => favoriteToolIds.value.map((id) => allToolMap.get(id)).filter((item): item is ToolDef => Boolean(item)));
const topMonthlyTools = computed(() => {
  const cutoff = Date.now() - CLICK_WINDOW_MS;
  const favoriteSet = new Set(favoriteToolIds.value);
  const stats = allTools
    .filter((tool) => !favoriteSet.has(tool.id))
    .map((tool) => {
      const clicks = (toolClickHistory.value[tool.id] ?? []).filter((timestamp) => timestamp >= cutoff).length;
      return { tool, count: clicks };
    })
    .filter((item) => item.count > 0)
    .sort((a, b) => b.count - a.count);
  return stats.slice(0, homeTopLimit.value);
});

const calcCurrentInput = ref("");
const calcHistory = ref<CalcDraftEntry[]>([]);
const calcCurrentPreview = computed(() => {
  return getCalcPreview(calcCurrentInput.value);
});
const cryptoInput = ref("");
const cryptoOutput = ref("");
const publicKeyPem = ref("");
const privateKeyPem = ref("");
const symmetricKey = ref("");
const symmetricIv = ref("");
const symmetricAlgorithm = ref("aes-256-cbc");

const formatInput = ref("");
const formatOutput = ref("");
type FormatKind = "json" | "xml" | "html" | "java" | "sql" | "plaintext";
const formatDetected = ref<FormatKind>("plaintext");
const monacoLanguage = computed(() => {
  const map: Record<string, string> = {
    json: "json",
    xml: "xml",
    html: "html",
    java: "java",
    sql: "sql"
  };
  return map[formatDetected.value] ?? "plaintext";
});
const formatDetectedLabel = computed(() => {
  const map: Record<FormatKind, string> = {
    json: "自动识别类型: JSON",
    xml: "自动识别类型: XML",
    html: "自动识别类型: HTML",
    java: "自动识别类型: Java",
    sql: "自动识别类型: SQL",
    plaintext: "自动识别类型: 未识别"
  };
  return map[formatDetected.value];
});

const convertInput = ref("");
const convertOutput = ref("");
const csvDelimiter = ref(",");

const textProcessInput = ref("");
const textProcessOutput = ref("");
const textCaseSensitive = ref(false);

const regexPattern = ref("");
const regexFlags = ref("g");
const regexInput = ref("");
const regexOutput = ref("");
const regexKind = ref<"email" | "ipv4" | "url" | "phone-cn">("email");
const regexTemplates = ref<unknown[]>([]);

const hostsName = ref("");
const hostsContent = ref("");
const hostsProfiles = ref<HostsProfile[]>([]);

const portUsageSummary = ref<PortUsageSummary>({ total: 0, tcp: 0, udp: 0 });
const portUsageStateRows = ref<PortUsageStateRow[]>([]);
const portProcessRows = ref<PortUsageProcessRow[]>([]);
const portConnectionRows = ref<PortUsageConnectionRow[]>([]);
const portFilter = ref("");
const filteredPortProcessRows = computed(() => {
  const needle = portFilter.value.trim();
  if (!needle) {
    return portProcessRows.value;
  }
  return portProcessRows.value.filter((row) => row.listeningPorts.some((port) => port.includes(needle)));
});
const envOutput = ref("");

const sourcePath = ref("");
const outputDir = ref("");
const chunkSizeMb = ref(100);
const partsInput = ref("");
const mergeOutputPath = ref("");
const fileToolOutput = ref("");

const imageInputPath = ref("");
const imageOutputPath = ref("");
const imageFormat = ref("png");
const imageWidth = ref(800);
const imageHeight = ref(800);
const cropX = ref(0);
const cropY = ref(0);
const cropWidth = ref(0);
const cropHeight = ref(0);
const imageQuality = ref(80);
const imageOutput = ref("");

const timeInput = ref("");
const timeOutput = ref("");
const timePrecision = ref<"s" | "ms">("s");
const dateInput = ref("");
const dateOutput = ref("");
const datePrecision = ref<"s" | "ms">("s");

const passwordLength = ref(20);
const passwordSymbols = ref(true);
const passwordNumbers = ref(true);
const passwordUppercase = ref(true);
const passwordLowercase = ref(true);
const idOutput = ref("");

const cronSecond = ref("0");
const cronMinute = ref("*");
const cronHour = ref("*");
const cronDom = ref("*");
const cronMonth = ref("*");
const cronDow = ref("*");
const cronExpression = ref("0 * * * * *");
const cronOutput = ref("");

function resetCommonBuffers() {
  // 各面板组件内部管理自身状态
}

function isRealToolId(id: string) {
  return allToolMap.has(id);
}

function isFavorite(id: string) {
  return favoriteToolIds.value.includes(id);
}

function toggleFavorite(id: string) {
  if (!isRealToolId(id)) {
    return;
  }
  if (isFavorite(id)) {
    favoriteToolIds.value = favoriteToolIds.value.filter((toolId) => toolId !== id);
    ElMessage.success("已取消收藏");
    return;
  }
  favoriteToolIds.value = [...favoriteToolIds.value, id];
  ElMessage.success("已加入收藏");
}

function openToolFromHome(id: string) {
  onSelect(id);
}

function persistFavorites() {
  localStorage.setItem(FAVORITE_STORAGE_KEY, JSON.stringify(favoriteToolIds.value));
}

function pruneClicks(history: ToolClickHistory): ToolClickHistory {
  const cutoff = Date.now() - CLICK_WINDOW_MS;
  const result: ToolClickHistory = {};
  for (const [toolId, timestamps] of Object.entries(history)) {
    if (!isRealToolId(toolId) || !Array.isArray(timestamps)) {
      continue;
    }
    const valid = timestamps
      .filter((item): item is number => typeof item === "number" && Number.isFinite(item) && item >= cutoff)
      .sort((a, b) => a - b)
      .slice(-MAX_CLICK_HISTORY_PER_TOOL);
    if (valid.length) {
      result[toolId] = valid;
    }
  }
  return result;
}

function persistClickHistory() {
  localStorage.setItem(TOOL_CLICKS_STORAGE_KEY, JSON.stringify(pruneClicks(toolClickHistory.value)));
}

function loadFavoritesFromStorage() {
  try {
    const raw = localStorage.getItem(FAVORITE_STORAGE_KEY);
    if (!raw) {
      favoriteToolIds.value = [];
      return;
    }
    const parsed = JSON.parse(raw);
    favoriteToolIds.value = Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string" && isRealToolId(id)) : [];
  } catch {
    favoriteToolIds.value = [];
  }
}

function loadClickHistoryFromStorage() {
  try {
    const raw = localStorage.getItem(TOOL_CLICKS_STORAGE_KEY);
    if (!raw) {
      toolClickHistory.value = {};
      return;
    }
    const parsed = JSON.parse(raw) as ToolClickHistory;
    toolClickHistory.value = pruneClicks(parsed);
  } catch {
    toolClickHistory.value = {};
  }
}

function loadHomeTopLimitFromStorage() {
  try {
    const raw = localStorage.getItem(HOME_TOP_LIMIT_STORAGE_KEY);
    if (raw === "6" || raw === "12") {
      homeTopLimit.value = Number(raw) as 6 | 12;
      return;
    }
    homeTopLimit.value = 12;
  } catch {
    homeTopLimit.value = 12;
  }
}

function persistCalcHistory() {
  localStorage.setItem(CALC_DRAFT_HISTORY_STORAGE_KEY, JSON.stringify(calcHistory.value));
}

function loadCalcHistoryFromStorage() {
  try {
    const raw = localStorage.getItem(CALC_DRAFT_HISTORY_STORAGE_KEY);
    if (!raw) {
      calcHistory.value = [];
      return;
    }
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      calcHistory.value = [];
      return;
    }
    calcHistory.value = parsed
      .filter((item): item is CalcDraftEntry => {
        return (
          typeof item?.id === "number" &&
          typeof item?.expression === "string" &&
          typeof item?.resultRaw === "string" &&
          typeof item?.resultDisplay === "string" &&
          typeof item?.createdAt === "number"
        );
      })
      .slice(-MAX_CALC_HISTORY);
  } catch {
    calcHistory.value = [];
  }
}

function normalizeExpression(input: string) {
  return input
    .replace(/[，,]/g, "")
    .replace(/、/g, "/")
    .replace(/[×xX]/g, "*")
    .replace(/÷/g, "/")
    .replace(/（/g, "(")
    .replace(/）/g, ")")
    .replace(/\s+/g, "")
    .replace(/(\d+(?:\.\d+)?)%/g, "($1/100)");
}

function formatCalcResult(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: 15
  }).format(value);
}

function calculateExpression(input: string): { rawValue: string; displayValue: string } {
  const normalized = normalizeExpression(input);
  if (!normalized) {
    throw new Error("请输入计算公式");
  }
  if (!/^[0-9+\-*/().]+$/.test(normalized)) {
    throw new Error("仅支持数字和 + - * / ( ) 运算符");
  }
  let result: unknown;
  try {
    result = Function(`"use strict"; return (${normalized});`)();
  } catch {
    throw new Error("公式格式不正确");
  }
  if (typeof result !== "number" || !Number.isFinite(result)) {
    throw new Error("计算结果无效");
  }
  return {
    rawValue: result.toString(),
    displayValue: formatCalcResult(result)
  };
}

function getCalcPreview(input: string) {
  const source = input.trim();
  if (!source) {
    return "";
  }

  try {
    return calculateExpression(source).displayValue;
  } catch {
    // If the current expression is incomplete (e.g. trailing operator),
    // fallback to the nearest valid prefix so preview stays useful.
  }

  const trailingSymbolPattern = /[+\-*/xX×÷、(]+$/;
  const fallbackSource = source.replace(trailingSymbolPattern, "").trim();
  if (!fallbackSource) {
    return "";
  }
  try {
    return calculateExpression(fallbackSource).displayValue;
  } catch {
    return "";
  }
}

async function copyTextToClipboard(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    return false;
  }
}

async function onCalcEnter() {
  try {
    const expression = calcCurrentInput.value.trim();
    if (!expression) {
      return;
    }
    const result = calculateExpression(expression);
    const now = Date.now();
    const entry: CalcDraftEntry = {
      id: now,
      expression,
      resultRaw: result.rawValue,
      resultDisplay: result.displayValue,
      createdAt: now
    };
    calcHistory.value = [...calcHistory.value, entry].slice(-MAX_CALC_HISTORY);
    const copied = await copyTextToClipboard(result.rawValue);
    calcCurrentInput.value = result.rawValue;
    ElMessage.success(copied ? `结果 ${result.displayValue} 已复制到剪贴板` : `结果 ${result.displayValue}（剪贴板写入失败）`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function clearCalcHistory() {
  calcHistory.value = [];
  ElMessage.success("计算历史已清空");
}

function onCalcInput(value: string) {
  calcCurrentInput.value = value.replaceAll("、", "/");
}

function formatCalcHistoryTime(timestamp: number) {
  const date = new Date(timestamp);
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
}

async function onCalcHistoryClick(item: CalcDraftEntry) {
  const copied = await copyTextToClipboard(item.resultRaw);
  ElMessage.success(copied ? `已复制结果: ${item.resultRaw}` : `复制失败，结果为: ${item.resultRaw}`);
}

function keepCalcViewportAtBottom() {
  if (activeTool.value !== "calc-draft") {
    return;
  }
  void nextTick(() => {
    const container = contentRef.value;
    if (!container) {
      return;
    }
    container.scrollTop = container.scrollHeight;
  });
}

function recordToolClick(id: string) {
  if (!isRealToolId(id)) {
    return;
  }
  const next = { ...toolClickHistory.value };
  const history = [...(next[id] ?? []), Date.now()];
  next[id] = history.slice(-MAX_CLICK_HISTORY_PER_TOOL);
  toolClickHistory.value = next;
}

function onSelect(id: string) {
  if (id !== HOME_ID) {
    recordToolClick(id);
  }
  activeTool.value = id;
  resetCommonBuffers();
  if (id === "timestamp") {
    timeInput.value = timePrecision.value === "s"
      ? String(Math.floor(Date.now() / 1000))
      : String(Date.now());
    timeOutput.value = "";
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    dateInput.value = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
    dateOutput.value = "";
  }
}

async function invoke(channel: string, payload: Record<string, unknown>) {
  return invokeToolByChannel(channel, payload);
}

async function rsaEncryptAction() {
  try {
    cryptoOutput.value = String(await invoke("tool:crypto:rsa-encrypt", { plaintext: cryptoInput.value, publicKeyPem: publicKeyPem.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function rsaDecryptAction() {
  try {
    cryptoOutput.value = String(await invoke("tool:crypto:rsa-decrypt", { cipherTextBase64: cryptoInput.value, privateKeyPem: privateKeyPem.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function symmetricEncrypt() {
  try {
    const channel = symmetricAlgorithm.value.startsWith("aes") ? "tool:crypto:aes-encrypt" : "tool:crypto:des-encrypt";
    cryptoOutput.value = String(
      await invoke(channel, {
        plaintext: cryptoInput.value,
        key: symmetricKey.value,
        iv: symmetricIv.value,
        algorithm: symmetricAlgorithm.value
      })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function symmetricDecrypt() {
  try {
    const channel = symmetricAlgorithm.value.startsWith("aes") ? "tool:crypto:aes-decrypt" : "tool:crypto:des-decrypt";
    cryptoOutput.value = String(
      await invoke(channel, {
        cipherTextBase64: cryptoInput.value,
        key: symmetricKey.value,
        iv: symmetricIv.value,
        algorithm: symmetricAlgorithm.value
      })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function formatCode() {
  try {
    const source = formatInput.value;
    if (!source.trim()) {
      formatOutput.value = "";
      formatDetected.value = "plaintext";
      return;
    }
    const detected = detectFormatKind(source);
    formatDetected.value = detected;
    if (detected === "plaintext") {
      throw new Error("无法识别代码类型，目前支持 JSON/XML/HTML/Java/SQL");
    }
    formatOutput.value = await formatByKind(source, detected);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function detectFormatKind(input: string): FormatKind {
  const source = input.trim();
  if (!source) {
    return "plaintext";
  }

  try {
    const parsed = JSON.parse(source);
    if (parsed !== undefined) {
      return "json";
    }
  } catch {
    // not json
  }

  if (source.startsWith("<") && source.endsWith(">")) {
    const lower = source.toLowerCase();
    if (
      lower.includes("<!doctype html") ||
      /<html[\s>]/i.test(source) ||
      /<(head|body|div|span|script|style|main|section|article|nav|footer|header)[\s>]/i.test(source)
    ) {
      return "html";
    }
    return "xml";
  }

  if (
    /\b(select|insert|update|delete|create|alter|drop|truncate|with)\b/i.test(source) &&
    /\b(from|into|table|where|values|set|join)\b/i.test(source)
  ) {
    return "sql";
  }

  if (
    /\b(class|interface|enum|record)\b/.test(source) &&
    /\b(public|private|protected|static|void|package|import)\b/.test(source)
  ) {
    return "java";
  }

  return "plaintext";
}

async function formatByKind(input: string, kind: Exclude<FormatKind, "plaintext">): Promise<string> {
  switch (kind) {
    case "json":
      return formatJson(input);
    case "xml":
      return formatXml(input);
    case "html":
      return formatHtml(input);
    case "java":
      return formatJava(input);
    case "sql":
      return formatSqlCode(input);
  }
}

async function runConvertTool(channel: string) {
  try {
    convertOutput.value = String(await invoke(channel, { input: convertInput.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function csvToJsonAction() {
  try {
    convertOutput.value = String(await invoke("tool:convert:csv-to-json", { input: convertInput.value, delimiter: csvDelimiter.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function dedupeLines() {
  try {
    textProcessOutput.value = String(
      await invoke("tool:text:unique-lines", { input: textProcessInput.value, caseSensitive: textCaseSensitive.value })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function sortTextLines() {
  try {
    textProcessOutput.value = String(
      await invoke("tool:text:sort-lines", { input: textProcessInput.value, caseSensitive: textCaseSensitive.value })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function runRegexTest() {
  try {
    const data = await invoke("tool:regex:test", {
      pattern: regexPattern.value,
      flags: regexFlags.value,
      input: regexInput.value
    });
    regexOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function applyRegexTemplate() {
  try {
    regexPattern.value = String(await invoke("tool:regex:generate", { kind: regexKind.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function loadRegexTemplates() {
  try {
    const data = await invoke("tool:regex:templates", {});
    regexTemplates.value = Array.isArray(data) ? data : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function loadHostsProfiles() {
  try {
    const data = await invoke("tool:hosts:list", {});
    hostsProfiles.value = Array.isArray(data) ? (data as HostsProfile[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function pickHosts(profile: HostsProfile) {
  hostsName.value = profile.name;
  hostsContent.value = profile.content;
}

async function saveHosts() {
  try {
    await invoke("tool:hosts:save", { name: hostsName.value, content: hostsContent.value });
    await loadHostsProfiles();
    ElMessage.success("hosts 配置已保存");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function activateHosts() {
  try {
    const data = await invoke("tool:hosts:activate", { profileName: hostsName.value, content: hostsContent.value });
    await loadHostsProfiles();
    ElMessage.success(`切换成功: ${JSON.stringify(data)}`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function deleteHosts() {
  try {
    await invoke("tool:hosts:delete", { name: hostsName.value });
    await loadHostsProfiles();
    ElMessage.success("hosts 配置已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function loadPortUsage() {
  try {
    const data = await invoke("tool:port:usage", {});
    const payload = (data ?? {}) as {
      summary?: { total?: number; tcp?: number; udp?: number };
      stateCounts?: Record<string, number>;
      processSummaries?: Array<{
        pid?: number;
        processName?: string;
        listeningPorts?: string[];
        connectionCount?: number;
      }>;
      connections?: Array<{
        protocol?: string;
        pid?: number;
        processName?: string;
        localAddress?: string;
        remoteAddress?: string;
        state?: string | null;
      }>;
    };
    const summary = payload.summary ?? {};
    const stateCounts = payload.stateCounts ?? {};
    const processSummaries = Array.isArray(payload.processSummaries) ? payload.processSummaries : [];
    const connections = Array.isArray(payload.connections) ? payload.connections : [];

    portUsageSummary.value = {
      total: summary.total ?? connections.length,
      tcp: summary.tcp ?? 0,
      udp: summary.udp ?? 0
    };
    portUsageStateRows.value = Object.entries(stateCounts)
      .map(([state, count]) => ({ state, count }))
      .sort((a, b) => b.count - a.count);
    portProcessRows.value = processSummaries.map((item) => ({
      pid: item.pid ?? 0,
      processName: item.processName ?? "UNKNOWN",
      listeningPorts: item.listeningPorts ?? [],
      listeningPortsText: (item.listeningPorts ?? []).join(", ") || "-",
      connectionCount: item.connectionCount ?? 0
    }));
    portConnectionRows.value = connections.slice(0, 1000).map((item) => ({
      protocol: item.protocol ?? "",
      pid: item.pid ?? 0,
      processName: item.processName ?? "UNKNOWN",
      localAddress: item.localAddress ?? "",
      remoteAddress: item.remoteAddress ?? "",
      state: item.state ?? "-"
    }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function detectEnv() {
  try {
    const data = await invoke("tool:env:detect", {});
    envOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function splitFile() {
  try {
    const data = await invoke("tool:file:split", {
      sourcePath: sourcePath.value,
      outputDir: outputDir.value,
      chunkSizeMb: chunkSizeMb.value
    });
    fileToolOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function mergeFiles() {
  try {
    const parts = partsInput.value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    const data = await invoke("tool:file:merge", { parts, outputPath: mergeOutputPath.value });
    fileToolOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function convertImageAction() {
  try {
    const data = await invoke("tool:image:convert", {
      inputPath: imageInputPath.value,
      outputPath: imageOutputPath.value,
      width: imageWidth.value,
      height: imageHeight.value,
      cropX: cropWidth.value > 0 ? cropX.value : undefined,
      cropY: cropHeight.value > 0 ? cropY.value : undefined,
      cropWidth: cropWidth.value > 0 ? cropWidth.value : undefined,
      cropHeight: cropHeight.value > 0 ? cropHeight.value : undefined,
      quality: imageQuality.value,
      format: imageFormat.value
    });
    imageOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function timestampToDate() {
  try {
    const ts = Number(timeInput.value);
    timeOutput.value = String(await invoke("tool:time:timestamp-to-date", { input: ts, timezone: "local" }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function dateToTimestamp() {
  try {
    const data = await invoke("tool:time:date-to-timestamp", { input: dateInput.value }) as { seconds: number; milliseconds: number };
    dateOutput.value = datePrecision.value === "s" ? String(data.seconds) : String(data.milliseconds);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function onDatePrecisionChange() {
  if (!dateOutput.value) return;
  const num = Number(dateOutput.value);
  if (!Number.isFinite(num)) return;
  if (datePrecision.value === "ms" && num < 1_000_000_000_000) {
    dateOutput.value = String(num * 1000);
  } else if (datePrecision.value === "s" && num >= 1_000_000_000_000) {
    dateOutput.value = String(Math.floor(num / 1000));
  }
}

function onTimePrecisionChange() {
  const val = timeInput.value.trim();
  if (/^\d+$/.test(val)) {
    const num = Number(val);
    if (timePrecision.value === "ms" && num < 1_000_000_000_000) {
      timeInput.value = String(num * 1000);
    } else if (timePrecision.value === "s" && num >= 1_000_000_000_000) {
      timeInput.value = String(Math.floor(num / 1000));
    }
  }
}

async function generateUuid() {
  try {
    idOutput.value = String(await invoke("tool:gen:uuid", {}));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generateGuidAction() {
  try {
    idOutput.value = String(await invoke("tool:gen:guid", {}));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generatePasswordAction() {
  try {
    idOutput.value = String(
      await invoke("tool:gen:password", {
        length: passwordLength.value,
        symbols: passwordSymbols.value,
        numbers: passwordNumbers.value,
        uppercase: passwordUppercase.value,
        lowercase: passwordLowercase.value
      })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

watch([cronSecond, cronMinute, cronHour, cronDom, cronMonth, cronDow], ([s, m, h, d, mo, dw]) => {
  cronExpression.value = `${s} ${m} ${h} ${d} ${mo} ${dw}`;
});

async function previewCron() {
  try {
    const data = await invoke("tool:cron:preview", { expression: cronExpression.value, count: 8 });
    cronOutput.value = (data as string[]).join("\n");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function parseCron() {
  try {
    const data = await invoke("tool:cron:parse", { expression: cronExpression.value }) as {
      second: string; minute: string; hour: string;
      dayOfMonth: string; month: string; dayOfWeek: string;
    };
    cronSecond.value = data.second;
    cronMinute.value = data.minute;
    cronHour.value   = data.hour;
    cronDom.value    = data.dayOfMonth;
    cronMonth.value  = data.month;
    cronDow.value    = data.dayOfWeek;
    ElMessage.success("解析成功");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

let autoProcessTimer: ReturnType<typeof setTimeout> | null = null;

function getAutoInputFingerprint(): string {
  switch (activeTool.value) {
    case "formatter":
      return `${activeTool.value}|${formatInput.value}`;
    case "json-xml":
    case "json-yaml":
      return `${activeTool.value}|${convertInput.value}`;
    case "csv-json":
      return `${activeTool.value}|${convertInput.value}|${csvDelimiter.value}`;
    case "text-process":
      return `${activeTool.value}|${textProcessInput.value}|${textCaseSensitive.value}`;
    case "regex":
      return `${activeTool.value}|${regexPattern.value}|${regexFlags.value}|${regexInput.value}`;
    case "network":
      return `${activeTool.value}|noop`;
    case "timestamp":
      return `${activeTool.value}|${timeInput.value}|${timePrecision.value}|${dateInput.value}|${datePrecision.value}`;
    default:
      return `${activeTool.value}|noop`;
  }
}

async function autoProcessByTool() {
  switch (activeTool.value) {
    case "formatter":
      await formatCode();
      return;
    case "json-xml":
      if (!convertInput.value.trim()) {
        convertOutput.value = "";
        return;
      }
      await runConvertTool("tool:convert:json-to-xml");
      return;
    case "json-yaml":
      if (!convertInput.value.trim()) {
        convertOutput.value = "";
        return;
      }
      await runConvertTool("tool:convert:json-to-yaml");
      return;
    case "csv-json":
      if (!convertInput.value.trim()) {
        convertOutput.value = "";
        return;
      }
      await csvToJsonAction();
      return;
    case "text-process":
      if (!textProcessInput.value.trim()) {
        textProcessOutput.value = "";
        return;
      }
      await dedupeLines();
      return;
    case "regex":
      if (!regexPattern.value.trim() && !regexInput.value.trim()) {
        regexOutput.value = "";
        return;
      }
      await runRegexTest();
      return;
    case "network":
      return;
    case "timestamp":
      if (timeInput.value.trim()) {
        await timestampToDate();
      }
      if (dateInput.value.trim()) {
        await dateToTimestamp();
      }
      return;
    default:
      return;
  }
}

watch(
  () => getAutoInputFingerprint(),
  (_next, prev) => {
    if (prev === undefined) {
      return;
    }
    if (autoProcessTimer) {
      clearTimeout(autoProcessTimer);
    }
    autoProcessTimer = setTimeout(() => {
      void autoProcessByTool();
    }, 300);
  }
);

watch(
  () => favoriteToolIds.value,
  () => {
    persistFavorites();
  },
  { deep: true }
);

watch(
  () => toolClickHistory.value,
  () => {
    persistClickHistory();
  },
  { deep: true }
);

watch(
  () => homeTopLimit.value,
  (value) => {
    localStorage.setItem(HOME_TOP_LIMIT_STORAGE_KEY, String(value));
  }
);

watch(
  () => calcHistory.value,
  () => {
    persistCalcHistory();
    keepCalcViewportAtBottom();
  },
  { deep: true }
);

watch(
  () => calcCurrentInput.value,
  () => {
    keepCalcViewportAtBottom();
  }
);

watch(
  () => activeTool.value,
  (value) => {
    if (value === "calc-draft") {
      keepCalcViewportAtBottom();
    }
  }
);

async function saveHotkeySettings() {
  const shortcut = hotkeyInput.value.trim();
  try {
    await registerHotkey(shortcut);
    localStorage.setItem(HOTKEY_STORAGE_KEY, shortcut);
    ElMessage.success(shortcut ? `快捷键 ${shortcut} 已保存` : "快捷键已清除");
  } catch (e) {
    ElMessage.error(`保存失败：${(e as Error).message}`);
  }
}

async function clearHotkeySettings() {
  hotkeyInput.value = "";
  try {
    await unregisterHotkey();
    localStorage.removeItem(HOTKEY_STORAGE_KEY);
    ElMessage.success("快捷键已清除");
  } catch (e) {
    ElMessage.error(`清除失败：${(e as Error).message}`);
  }
}

onMounted(async () => {
  loadFavoritesFromStorage();
  loadClickHistoryFromStorage();
  loadHomeTopLimitFromStorage();
  loadCalcHistoryFromStorage();
  const savedHotkey = localStorage.getItem(HOTKEY_STORAGE_KEY) ?? "";
  hotkeyInput.value = savedHotkey;
  if (savedHotkey) {
    try { await registerHotkey(savedHotkey); } catch { /* ignore in non-Tauri env */ }
  }
  await Promise.all([loadHostsProfiles(), loadRegexTemplates()]);
});
</script>


