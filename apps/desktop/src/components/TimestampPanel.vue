<template>
  <div class="panel-grid">
    <div class="panel-grid-full row">
      <span class="row-label">时间戳 -> 日期</span>
      <el-input v-model="timeInput" placeholder="时间戳" style="flex: 1" />
      <el-button-group>
        <el-button :type="timePrecision === 's' ? 'primary' : ''" @click="setTimePrecision('s')">
          秒
        </el-button>
        <el-button :type="timePrecision === 'ms' ? 'primary' : ''" @click="setTimePrecision('ms')">
          毫秒
        </el-button>
      </el-button-group>
      <el-input v-model="timeOutput" readonly placeholder="日期结果" style="flex: 1" />
    </div>

    <div class="panel-grid-full row">
      <span class="row-label">日期 -> 时间戳</span>
      <el-input
        v-model="dateInput"
        placeholder="日期，如 2026-02-21 10:20:30"
        style="flex: 1"
      />
      <el-button-group>
        <el-button :type="datePrecision === 's' ? 'primary' : ''" @click="setDatePrecision('s')">
          秒
        </el-button>
        <el-button :type="datePrecision === 'ms' ? 'primary' : ''" @click="setDatePrecision('ms')">
          毫秒
        </el-button>
      </el-button-group>
      <el-input v-model="dateOutput" readonly placeholder="时间戳结果" style="flex: 1" />
    </div>

    <div class="panel-grid-full java-wrap">
      <div class="java-head">
        <span class="java-title">Java 日期时间互转工具栏</span>
        <span class="java-hint">支持 Date / Instant / LocalDate / LocalDateTime / ZonedDateTime</span>
      </div>

      <div class="java-toolbar">
        <el-select v-model="javaSourceType" size="small" style="width: 190px">
          <el-option
            v-for="opt in javaTypeOptions"
            :key="opt.value"
            :label="`来源：${opt.label}`"
            :value="opt.value"
          />
        </el-select>
        <el-select v-model="javaTargetType" size="small" style="width: 190px">
          <el-option
            v-for="opt in javaTypeOptions"
            :key="opt.value"
            :label="`目标：${opt.label}`"
            :value="opt.value"
          />
        </el-select>
        <el-button size="small" type="primary" @click="convertJavaDateTime">转换值</el-button>
        <el-button size="small" @click="generateJavaCode">生成 Java 代码</el-button>
        <el-button size="small" @click="fillJavaFromCurrent">取当前值</el-button>
      </div>

      <el-input
        v-model="javaInput"
        type="textarea"
        :rows="2"
        placeholder="输入要转换的值（可留空，仅生成模板代码）"
      />
      <el-input
        v-model="javaOutput"
        type="textarea"
        :rows="2"
        readonly
        placeholder="转换结果"
      />
      <el-input
        v-model="javaCodeOutput"
        type="textarea"
        :rows="8"
        readonly
        placeholder="Java 转换代码"
      />

      <div class="java-toolbar">
        <el-button size="small" @click="copyJavaOutput">复制结果</el-button>
        <el-button size="small" @click="copyJavaCode">复制代码</el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

type Precision = "s" | "ms";
type JavaDateType =
  | "timestamp_s"
  | "timestamp_ms"
  | "date"
  | "instant"
  | "local_date"
  | "local_datetime"
  | "zoned_datetime";

const javaTypeOptions: Array<{ value: JavaDateType; label: string }> = [
  { value: "timestamp_s", label: "Timestamp (秒)" },
  { value: "timestamp_ms", label: "Timestamp (毫秒)" },
  { value: "date", label: "Date" },
  { value: "instant", label: "Instant" },
  { value: "local_date", label: "LocalDate" },
  { value: "local_datetime", label: "LocalDateTime" },
  { value: "zoned_datetime", label: "ZonedDateTime" },
];

const timeInput = ref("");
const timeOutput = ref("");
const timePrecision = ref<Precision>("s");

const dateInput = ref("");
const dateOutput = ref("");
const datePrecision = ref<Precision>("s");

const javaInput = ref("");
const javaOutput = ref("");
const javaCodeOutput = ref("");
const javaSourceType = ref<JavaDateType>("date");
const javaTargetType = ref<JavaDateType>("local_datetime");

const now = new Date();
timeInput.value = String(Math.floor(now.getTime() / 1000));
dateInput.value = formatLocalDateTime(now);

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function formatLocalDate(date: Date): string {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

function formatLocalDateTime(date: Date): string {
  return `${formatLocalDate(date)} ${pad2(date.getHours())}:${pad2(date.getMinutes())}:${pad2(date.getSeconds())}`;
}

function parseLocalDate(raw: string): Date | null {
  const match = raw.trim().match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return null;
  const [, y, m, d] = match;
  const date = new Date(Number(y), Number(m) - 1, Number(d), 0, 0, 0, 0);
  return Number.isNaN(date.getTime()) ? null : date;
}

function parseLocalDateTime(raw: string): Date | null {
  const normalized = raw.trim().replace("T", " ");
  const match = normalized.match(
    /^(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2})(?::(\d{2}))?$/,
  );
  if (!match) return null;
  const [, y, m, d, hh, mm, ss] = match;
  const date = new Date(
    Number(y),
    Number(m) - 1,
    Number(d),
    Number(hh),
    Number(mm),
    Number(ss ?? "0"),
    0,
  );
  return Number.isNaN(date.getTime()) ? null : date;
}

function parseByType(raw: string, type: JavaDateType): Date | null {
  const text = raw.trim();
  if (!text) return null;

  if (type === "timestamp_s") {
    const value = Number(text);
    if (!Number.isFinite(value)) return null;
    return new Date(value * 1000);
  }

  if (type === "timestamp_ms") {
    const value = Number(text);
    if (!Number.isFinite(value)) return null;
    return new Date(value);
  }

  if (type === "local_date") return parseLocalDate(text);
  if (type === "local_datetime") return parseLocalDateTime(text);

  if (type === "date" || type === "instant" || type === "zoned_datetime") {
    const date = new Date(text);
    return Number.isNaN(date.getTime()) ? null : date;
  }

  return null;
}

function formatByType(date: Date, type: JavaDateType): string {
  if (type === "timestamp_s") return String(Math.floor(date.getTime() / 1000));
  if (type === "timestamp_ms") return String(date.getTime());
  if (type === "local_date") return formatLocalDate(date);
  if (type === "local_datetime") return formatLocalDateTime(date);
  return date.toISOString();
}

function setTimePrecision(next: Precision) {
  timePrecision.value = next;
  const value = timeInput.value.trim();
  if (!/^\d+$/.test(value)) return;
  const num = Number(value);
  if (next === "ms" && num < 1_000_000_000_000) timeInput.value = String(num * 1000);
  if (next === "s" && num >= 1_000_000_000_000) timeInput.value = String(Math.floor(num / 1000));
}

function setDatePrecision(next: Precision) {
  datePrecision.value = next;
  if (!dateOutput.value) return;
  const num = Number(dateOutput.value);
  if (!Number.isFinite(num)) return;
  if (next === "ms" && num < 1_000_000_000_000) dateOutput.value = String(num * 1000);
  if (next === "s" && num >= 1_000_000_000_000) dateOutput.value = String(Math.floor(num / 1000));
}

async function timestampToDate() {
  try {
    const raw = timeInput.value.trim();
    if (!raw) {
      timeOutput.value = "";
      return;
    }

    const ts = Number(raw);
    if (!Number.isFinite(ts)) {
      timeOutput.value = "";
      return;
    }

    const normalized = timePrecision.value === "s" ? ts : Math.floor(ts / 1000);
    timeOutput.value = String(
      await invokeToolByChannel("tool:time:timestamp-to-date", { input: normalized, timezone: "local" }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function dateToTimestamp() {
  try {
    const raw = dateInput.value.trim();
    if (!raw) {
      dateOutput.value = "";
      return;
    }

    const data = (await invokeToolByChannel("tool:time:date-to-timestamp", {
      input: raw,
    })) as { seconds: number; milliseconds: number };

    dateOutput.value = datePrecision.value === "s" ? String(data.seconds) : String(data.milliseconds);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function fillJavaFromCurrent() {
  if (javaSourceType.value === "timestamp_s") {
    javaInput.value =
      timePrecision.value === "s"
        ? timeInput.value.trim()
        : String(Math.floor(Number(timeInput.value || 0) / 1000));
    return;
  }

  if (javaSourceType.value === "timestamp_ms") {
    javaInput.value =
      timePrecision.value === "ms"
        ? timeInput.value.trim()
        : String(Number(timeInput.value || 0) * 1000);
    return;
  }

  const parsed = parseLocalDateTime(dateInput.value) ?? parseLocalDate(dateInput.value);
  if (!parsed) {
    javaInput.value = "";
    return;
  }

  if (javaSourceType.value === "local_date") {
    javaInput.value = formatLocalDate(parsed);
    return;
  }

  if (javaSourceType.value === "local_datetime") {
    javaInput.value = formatLocalDateTime(parsed).replace(" ", "T");
    return;
  }

  javaInput.value = parsed.toISOString();
}

function convertJavaDateTime() {
  if (!javaInput.value.trim()) {
    javaOutput.value = "";
    return;
  }

  const parsed = parseByType(javaInput.value, javaSourceType.value);
  if (!parsed) {
    javaOutput.value = "";
    ElMessage.warning("输入格式不合法，请检查来源类型和输入内容");
    return;
  }

  javaOutput.value = formatByType(parsed, javaTargetType.value);
}

function classNameByType(type: JavaDateType): string {
  if (type === "timestamp_s") return "long";
  if (type === "timestamp_ms") return "long";
  if (type === "date") return "Date";
  if (type === "instant") return "Instant";
  if (type === "local_date") return "LocalDate";
  if (type === "local_datetime") return "LocalDateTime";
  return "ZonedDateTime";
}

function varNameByType(type: JavaDateType): string {
  if (type === "timestamp_s") return "timestampSeconds";
  if (type === "timestamp_ms") return "timestampMillis";
  if (type === "date") return "date";
  if (type === "instant") return "instant";
  if (type === "local_date") return "localDate";
  if (type === "local_datetime") return "localDateTime";
  return "zonedDateTime";
}

function sourceInitLine(type: JavaDateType, input: string): string {
  const text = input.trim();
  if (type === "timestamp_s") return `long timestampSeconds = ${text || "1700000000"}L;`;
  if (type === "timestamp_ms") return `long timestampMillis = ${text || "1700000000000"}L;`;
  if (type === "date") {
    if (/^\d+$/.test(text)) return `Date date = new Date(${text}L);`;
    return "Date date = new Date();";
  }
  if (type === "instant") return `Instant instant = Instant.parse("${text || "2026-02-21T10:20:30Z"}");`;
  if (type === "local_date") return `LocalDate localDate = LocalDate.parse("${text || "2026-02-21"}");`;
  if (type === "local_datetime") {
    const value = text || "2026-02-21T10:20:30";
    return `LocalDateTime localDateTime = LocalDateTime.parse("${value.replace(" ", "T")}");`;
  }
  return `ZonedDateTime zonedDateTime = ZonedDateTime.parse("${text || "2026-02-21T10:20:30+08:00[Asia/Shanghai]"}");`;
}

function toInstantExpression(type: JavaDateType, sourceVar: string): string {
  if (type === "timestamp_s") return `Instant.ofEpochSecond(${sourceVar})`;
  if (type === "timestamp_ms") return `Instant.ofEpochMilli(${sourceVar})`;
  if (type === "date") return `${sourceVar}.toInstant()`;
  if (type === "local_date") return `${sourceVar}.atStartOfDay(zoneId).toInstant()`;
  if (type === "local_datetime") return `${sourceVar}.atZone(zoneId).toInstant()`;
  if (type === "zoned_datetime") return `${sourceVar}.toInstant()`;
  return sourceVar;
}

function fromInstantExpression(type: JavaDateType, instantVar: string): string {
  if (type === "timestamp_s") return `${instantVar}.getEpochSecond()`;
  if (type === "timestamp_ms") return `${instantVar}.toEpochMilli()`;
  if (type === "date") return `Date.from(${instantVar})`;
  if (type === "local_date") return `${instantVar}.atZone(zoneId).toLocalDate()`;
  if (type === "local_datetime") return `${instantVar}.atZone(zoneId).toLocalDateTime()`;
  if (type === "zoned_datetime") return `${instantVar}.atZone(zoneId)`;
  return instantVar;
}

function needsZone(type: JavaDateType): boolean {
  return type === "local_date" || type === "local_datetime" || type === "zoned_datetime";
}

function generateJavaCode() {
  const sourceType = javaSourceType.value;
  const targetType = javaTargetType.value;
  const lines: string[] = [];
  const sourceVar = varNameByType(sourceType);
  const targetVar = varNameByType(targetType);

  lines.push("// import java.time.*;");
  lines.push("// import java.util.Date;");
  lines.push(sourceInitLine(sourceType, javaInput.value));

  if (sourceType === targetType) {
    lines.push(`// 源类型与目标类型一致，直接使用 ${sourceVar}`);
    javaCodeOutput.value = lines.join("\n");
    return;
  }

  const requireZone = needsZone(sourceType) || needsZone(targetType);
  if (requireZone) {
    lines.push("ZoneId zoneId = ZoneId.systemDefault();");
  }

  const instantVarName = sourceType === "instant" ? sourceVar : "instant";
  if (sourceType !== "instant") {
    lines.push(`Instant ${instantVarName} = ${toInstantExpression(sourceType, sourceVar)};`);
  }

  if (targetType !== "instant") {
    lines.push(
      `${classNameByType(targetType)} ${targetVar} = ${fromInstantExpression(targetType, instantVarName)};`,
    );
  }

  javaCodeOutput.value = lines.join("\n");
}

async function copyJavaOutput() {
  if (!javaOutput.value) return;
  try {
    await navigator.clipboard.writeText(javaOutput.value);
    ElMessage.success("已复制结果");
  } catch {
    ElMessage.error("复制结果失败，请手动复制");
  }
}

async function copyJavaCode() {
  if (!javaCodeOutput.value) return;
  try {
    await navigator.clipboard.writeText(javaCodeOutput.value);
    ElMessage.success("已复制代码");
  } catch {
    ElMessage.error("复制代码失败，请手动复制");
  }
}

let timer: ReturnType<typeof setTimeout> | null = null;
watch([timeInput, timePrecision, dateInput, datePrecision], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    void timestampToDate();
    void dateToTimestamp();
  }, 250);
});

watch([javaInput, javaSourceType, javaTargetType], () => {
  convertJavaDateTime();
  generateJavaCode();
});
</script>

<style scoped>
.row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.row-label {
  white-space: nowrap;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

.java-wrap {
  display: grid;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--lc-border);
  border-radius: 10px;
}

.java-head {
  display: flex;
  gap: 10px;
  align-items: baseline;
  flex-wrap: wrap;
}

.java-title {
  font-size: 13px;
  color: var(--el-text-color-primary);
}

.java-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.java-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}
</style>
