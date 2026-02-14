<template>
  <div class="shell">
    <aside class="nav">
      <el-text tag="h2" size="large">Lazycat 懒猫</el-text>
      <el-menu :default-active="activeTool" @select="onSelect">
        <el-sub-menu v-for="group in groups" :key="group.id" :index="group.id">
          <template #title>{{ group.name }}</template>
          <el-menu-item v-for="tool in group.tools" :key="tool.id" :index="tool.id">
            {{ tool.name }}
          </el-menu-item>
        </el-sub-menu>
      </el-menu>
    </aside>

    <main class="content">
      <h1 class="tool-title">{{ currentTool?.name }}</h1>
      <p class="tool-desc">{{ currentTool?.desc }}</p>

      <div v-if="activeTool === 'base64'" class="panel-grid">
        <el-input v-model="textInput" type="textarea" :rows="10" placeholder="输入文本" />
        <el-input :model-value="textOutput" type="textarea" :rows="10" readonly placeholder="结果" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="runTextTool('tool:encode:base64-encode')">Base64 编码</el-button>
            <el-button @click="runTextTool('tool:encode:base64-decode')">Base64 解码</el-button>
          </el-space>
        </div>
      </div>

      <div v-else-if="activeTool === 'url'" class="panel-grid">
        <el-input v-model="textInput" type="textarea" :rows="10" placeholder="输入 URL 文本" />
        <el-input :model-value="textOutput" type="textarea" :rows="10" readonly placeholder="结果" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="runTextTool('tool:encode:url-encode')">URL 编码</el-button>
            <el-button @click="runTextTool('tool:encode:url-decode')">URL 解码</el-button>
          </el-space>
        </div>
      </div>

      <div v-else-if="activeTool === 'md5'" class="panel-grid">
        <el-input v-model="textInput" type="textarea" :rows="10" placeholder="输入文本" />
        <el-input :model-value="textOutput" type="textarea" :rows="10" readonly placeholder="MD5 结果" />
        <div class="panel-grid-full">
          <el-button type="primary" @click="runTextTool('tool:encode:md5')">计算 MD5</el-button>
        </div>
      </div>

      <div v-else-if="activeTool === 'qr'" class="panel-grid">
        <el-input v-model="textInput" type="textarea" :rows="7" placeholder="输入文本并生成二维码" />
        <div class="qr-preview">
          <img v-if="qrDataUrl" :src="qrDataUrl" alt="QR code" class="qr-image" />
          <el-empty v-else description="尚未生成二维码" />
        </div>
        <el-input class="panel-grid-full" :model-value="qrDataUrl" type="textarea" :rows="4" readonly placeholder="Data URL" />
        <div class="panel-grid-full">
          <el-button type="primary" @click="generateQr">生成二维码</el-button>
        </div>
      </div>

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

      <div v-else-if="activeTool === 'formatter'" class="panel-grid">
        <MonacoPane v-model="formatInput" :language="monacoLanguage" />
        <MonacoPane v-model="formatOutput" :language="monacoLanguage" :read-only="true" />
        <div>
          <el-button type="primary" @click="formatCode">执行格式化</el-button>
        </div>
        <el-input :model-value="formatDetectedLabel" readonly />
      </div>

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

      <div v-else-if="activeTool === 'regex'" class="panel-grid">
        <el-input v-model="regexPattern" placeholder="正则表达式" />
        <el-input v-model="regexFlags" placeholder="flags，例如 gi" />
        <el-input v-model="regexInput" class="panel-grid-full" type="textarea" :rows="8" placeholder="待匹配文本" />
        <el-input v-model="regexOutput" class="panel-grid-full" type="textarea" :rows="8" readonly placeholder="匹配结果" />
        <el-select v-model="regexKind" placeholder="常用模板">
          <el-option label="邮箱" value="email" />
          <el-option label="IPv4" value="ipv4" />
          <el-option label="URL" value="url" />
          <el-option label="中国手机号" value="phone-cn" />
        </el-select>
        <div>
          <el-space>
            <el-button type="primary" @click="runRegexTest">执行匹配</el-button>
            <el-button @click="applyRegexTemplate">填充模板</el-button>
            <el-button @click="loadRegexTemplates">查看模板库</el-button>
          </el-space>
        </div>
        <el-input
          class="panel-grid-full"
          :model-value="JSON.stringify(regexTemplates, null, 2)"
          type="textarea"
          :rows="6"
          readonly
          placeholder="模板库"
        />
      </div>

      <div v-else-if="activeTool === 'network'" class="panel-grid">
        <el-input v-model="host" placeholder="host" />
        <el-input-number v-model="port" :min="1" :max="65535" />
        <el-input-number v-model="timeoutMs" :min="100" :max="10000" />
        <div>
          <el-button type="primary" @click="runNetworkTest">测试连通性</el-button>
        </div>
        <el-input class="panel-grid-full" v-model="networkOutput" type="textarea" :rows="8" readonly />
      </div>

      <div v-else-if="activeTool === 'hosts'" class="panel-grid">
        <el-input v-model="hostsName" placeholder="配置名称，例如 local-dev" />
        <el-input v-model="hostsContent" type="textarea" :rows="8" placeholder="hosts 内容" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="saveHosts">保存配置</el-button>
            <el-button @click="activateHosts">设为当前配置</el-button>
            <el-button type="danger" @click="deleteHosts">删除配置</el-button>
            <el-button @click="loadHostsProfiles">刷新列表</el-button>
          </el-space>
        </div>
        <el-table class="panel-grid-full" :data="hostsProfiles" border>
          <el-table-column prop="name" label="名称" />
          <el-table-column prop="enabled" label="启用" width="80">
            <template #default="{ row }">{{ row.enabled ? "Yes" : "No" }}</template>
          </el-table-column>
          <el-table-column prop="updatedAt" label="更新时间" />
          <el-table-column label="操作" width="110">
            <template #default="{ row }">
              <el-button size="small" @click="pickHosts(row)">载入</el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <div v-else-if="activeTool === 'ports'" class="panel-grid">
        <div class="panel-grid-full">
          <el-button type="primary" @click="loadPortUsage">查询端口占用</el-button>
        </div>
        <el-divider class="panel-grid-full" content-position="left">概览</el-divider>
        <el-descriptions class="panel-grid-full" :column="3" border>
          <el-descriptions-item label="总连接">{{ portUsageSummary.total }}</el-descriptions-item>
          <el-descriptions-item label="TCP">{{ portUsageSummary.tcp }}</el-descriptions-item>
          <el-descriptions-item label="UDP">{{ portUsageSummary.udp }}</el-descriptions-item>
        </el-descriptions>
        <el-table class="panel-grid-full" :data="portUsageStateRows" border>
          <el-table-column prop="state" label="状态" />
          <el-table-column prop="count" label="数量" width="120" />
        </el-table>
        <el-divider class="panel-grid-full" content-position="left">按应用汇总</el-divider>
        <el-input
          v-model="portFilter"
          class="panel-grid-full"
          placeholder="按端口筛选应用汇总，例如 5173"
          clearable
        />
        <el-table class="panel-grid-full" :data="filteredPortProcessRows" border max-height="280">
          <el-table-column prop="processName" label="应用" min-width="180" />
          <el-table-column prop="pid" label="PID" width="100" />
          <el-table-column prop="listeningPortsText" label="监听端口" min-width="220" />
          <el-table-column prop="connectionCount" label="连接数" width="120" />
        </el-table>
        <el-divider class="panel-grid-full" content-position="left">连接明细</el-divider>
        <el-table class="panel-grid-full" :data="portConnectionRows" border max-height="360">
          <el-table-column prop="protocol" label="协议" width="90" />
          <el-table-column prop="pid" label="PID" width="90" />
          <el-table-column prop="processName" label="应用" min-width="180" />
          <el-table-column prop="localAddress" label="本地地址" min-width="220" />
          <el-table-column prop="remoteAddress" label="远端地址" min-width="220" />
          <el-table-column prop="state" label="状态" width="130" />
        </el-table>
      </div>

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
        <el-input v-model="timeInput" placeholder="输入时间戳（秒/毫秒）或日期字符串" />
        <el-input v-model="timeOutput" readonly />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="timestampToDate">时间戳 -> 日期</el-button>
            <el-button @click="dateToTimestamp">日期 -> 时间戳</el-button>
          </el-space>
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
        <el-input v-model="cronSecond" placeholder="秒，默认 0" />
        <el-input v-model="cronMinute" placeholder="分，默认 *" />
        <el-input v-model="cronHour" placeholder="时，默认 *" />
        <el-input v-model="cronDom" placeholder="日，默认 *" />
        <el-input v-model="cronMonth" placeholder="月，默认 *" />
        <el-input v-model="cronDow" placeholder="周，默认 *" />
        <el-input class="panel-grid-full" v-model="cronExpression" placeholder="Cron 表达式" />
        <div class="panel-grid-full">
          <el-space>
            <el-button type="primary" @click="generateCron">生成表达式</el-button>
            <el-button @click="previewCron">预览触发时间</el-button>
          </el-space>
        </div>
        <el-input class="panel-grid-full" v-model="cronOutput" type="textarea" :rows="8" readonly />
      </div>

      <div v-else-if="activeTool === 'manuals'" class="panel-grid">
        <div class="panel-grid-full">
          <el-button type="primary" @click="loadManuals">刷新离线手册列表</el-button>
        </div>
        <el-table class="panel-grid-full" :data="manuals" border>
          <el-table-column prop="name" label="手册名称" />
          <el-table-column prop="path" label="离线路径" />
        </el-table>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import MonacoPane from "./components/MonacoPane.vue";
import { invokeToolByChannel } from "./bridge/tauri";
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
      { id: "timestamp", name: "时间戳转换", desc: "时间戳与日期互转" },
      { id: "uuid", name: "UUID/GUID/密码", desc: "标识与随机密码生成" },
      { id: "cron", name: "Cron 工具", desc: "Cron 表达式生成与预览" },
      { id: "manuals", name: "离线手册", desc: "Vue2/Vue3/Element Plus 索引" }
    ]
  }
];

const activeTool = ref("base64");
const currentTool = computed(() => groups.flatMap((g) => g.tools).find((tool) => tool.id === activeTool.value));

const textInput = ref("");
const textOutput = ref("");
const qrDataUrl = ref("");

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

const host = ref("127.0.0.1");
const port = ref(80);
const timeoutMs = ref(2000);
const networkOutput = ref("");

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

const manuals = ref<Array<{ id: string; name: string; path: string }>>([]);

function resetCommonBuffers() {
  textOutput.value = "";
  qrDataUrl.value = "";
}

function onSelect(id: string) {
  activeTool.value = id;
  resetCommonBuffers();
}

async function invoke(channel: string, payload: Record<string, unknown>) {
  return invokeToolByChannel(channel, payload);
}

async function runTextTool(channel: string) {
  try {
    const data = await invoke(channel, { input: textInput.value });
    textOutput.value = typeof data === "string" ? data : JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generateQr() {
  try {
    const data = await invoke("tool:encode:qr", { input: textInput.value });
    qrDataUrl.value = String(data);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
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

async function runNetworkTest() {
  try {
    const data = await invoke("tool:network:tcp-test", {
      host: host.value,
      port: port.value,
      timeoutMs: timeoutMs.value
    });
    networkOutput.value = JSON.stringify(data, null, 2);
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
    const data = await invoke("tool:time:date-to-timestamp", { input: timeInput.value });
    timeOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
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

async function generateCron() {
  try {
    cronExpression.value = String(
      await invoke("tool:cron:generate", {
        second: cronSecond.value,
        minute: cronMinute.value,
        hour: cronHour.value,
        dayOfMonth: cronDom.value,
        month: cronMonth.value,
        dayOfWeek: cronDow.value
      })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function previewCron() {
  try {
    const data = await invoke("tool:cron:preview", { expression: cronExpression.value, count: 8 });
    cronOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function loadManuals() {
  try {
    const data = await invoke("tool:manuals:list", {});
    manuals.value = Array.isArray(data) ? (data as Array<{ id: string; name: string; path: string }>) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

let autoProcessTimer: ReturnType<typeof setTimeout> | null = null;

function getAutoInputFingerprint(): string {
  switch (activeTool.value) {
    case "base64":
    case "url":
    case "md5":
    case "qr":
      return `${activeTool.value}|${textInput.value}`;
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
      return `${activeTool.value}|${host.value}|${port.value}|${timeoutMs.value}`;
    case "timestamp":
      return `${activeTool.value}|${timeInput.value}`;
    default:
      return `${activeTool.value}|noop`;
  }
}

async function autoProcessByTool() {
  switch (activeTool.value) {
    case "base64":
      if (!textInput.value.trim()) {
        textOutput.value = "";
        return;
      }
      await runTextTool("tool:encode:base64-encode");
      return;
    case "url":
      if (!textInput.value.trim()) {
        textOutput.value = "";
        return;
      }
      await runTextTool("tool:encode:url-encode");
      return;
    case "md5":
      if (!textInput.value.trim()) {
        textOutput.value = "";
        return;
      }
      await runTextTool("tool:encode:md5");
      return;
    case "qr":
      if (!textInput.value.trim()) {
        qrDataUrl.value = "";
        return;
      }
      await generateQr();
      return;
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
      if (!host.value.trim()) {
        networkOutput.value = "";
        return;
      }
      await runNetworkTest();
      return;
    case "timestamp":
      if (!timeInput.value.trim()) {
        timeOutput.value = "";
        return;
      }
      if (/^\d+$/.test(timeInput.value.trim())) {
        await timestampToDate();
      } else {
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

onMounted(async () => {
  await Promise.all([loadHostsProfiles(), loadRegexTemplates(), loadManuals()]);
});
</script>


