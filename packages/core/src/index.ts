import { createHash, randomInt, randomUUID } from "node:crypto";
import { XMLBuilder, XMLParser } from "fast-xml-parser";
import { CronExpressionParser } from "cron-parser";
import QRCode from "qrcode";
import YAML from "yaml";

export function encodeBase64(input: string): string {
  return Buffer.from(input, "utf8").toString("base64");
}

export function decodeBase64(input: string): string {
  return Buffer.from(input, "base64").toString("utf8");
}

export function urlEncode(input: string): string {
  return encodeURIComponent(input);
}

export function urlDecode(input: string): string {
  return decodeURIComponent(input);
}

export function md5(input: string): string {
  return createHash("md5").update(input, "utf8").digest("hex");
}

export function jsonToXml(input: string): string {
  const parsed = JSON.parse(input);
  const builder = new XMLBuilder({
    ignoreAttributes: false,
    format: true
  });
  return builder.build(parsed);
}

export function xmlToJson(input: string): string {
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: "@_"
  });
  const parsed = parser.parse(input);
  return JSON.stringify(parsed, null, 2);
}

export function jsonToYaml(input: string): string {
  const parsed = JSON.parse(input);
  return YAML.stringify(parsed);
}

export function csvToJson(input: string, delimiter = ","): string {
  const lines = input
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) {
    return "[]";
  }
  const headers = splitCsvLine(lines[0], delimiter);
  const records = lines.slice(1).map((line) => {
    const values = splitCsvLine(line, delimiter);
    return headers.reduce<Record<string, string>>((acc, header, index) => {
      acc[header] = values[index] ?? "";
      return acc;
    }, {});
  });
  return JSON.stringify(records, null, 2);
}

function splitCsvLine(line: string, delimiter: string): string[] {
  const fields: string[] = [];
  let current = "";
  let quoted = false;

  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (c === "\"") {
      quoted = !quoted;
      continue;
    }
    if (c === delimiter && !quoted) {
      fields.push(current);
      current = "";
      continue;
    }
    current += c;
  }
  fields.push(current);
  return fields;
}

export function uniqueLines(input: string, caseSensitive = false): string {
  const seen = new Set<string>();
  const lines = input.split(/\r?\n/);
  const out: string[] = [];

  for (const line of lines) {
    const key = caseSensitive ? line : line.toLowerCase();
    if (!seen.has(key)) {
      seen.add(key);
      out.push(line);
    }
  }
  return out.join("\n");
}

export function sortLines(input: string, caseSensitive = false): string {
  const lines = input.split(/\r?\n/).filter((line) => line.length > 0);
  const collator = new Intl.Collator("zh-CN", { sensitivity: caseSensitive ? "variant" : "base", numeric: true });
  return lines.sort((a, b) => collator.compare(a, b)).join("\n");
}

export function timestampToDateTime(timestamp: number, timezone: "local" | "utc" = "local"): string {
  const ts = timestamp < 1000000000000 ? timestamp * 1000 : timestamp;
  const d = new Date(ts);
  return timezone === "utc" ? d.toISOString() : `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

export function dateTimeToTimestamp(input: string): { seconds: number; milliseconds: number } {
  const d = new Date(input);
  if (Number.isNaN(d.getTime())) {
    throw new Error("Invalid date-time string.");
  }
  return {
    seconds: Math.floor(d.getTime() / 1000),
    milliseconds: d.getTime()
  };
}

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

export function generateUuidV4(): string {
  return randomUUID();
}

export function generateGuid(): string {
  return `{${randomUUID().toUpperCase()}}`;
}

export interface PasswordOptions {
  length?: number;
  uppercase?: boolean;
  lowercase?: boolean;
  numbers?: boolean;
  symbols?: boolean;
  excludeSimilar?: boolean;
}

export function generatePassword(options: PasswordOptions): string {
  const length = options.length ?? 16;
  const includeUpper = options.uppercase ?? true;
  const includeLower = options.lowercase ?? true;
  const includeNumbers = options.numbers ?? true;
  const includeSymbols = options.symbols ?? false;
  const excludeSimilar = options.excludeSimilar ?? false;

  const upper = excludeSimilar ? "ABCDEFGHJKLMNPQRSTUVWXYZ" : "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
  const lower = excludeSimilar ? "abcdefghijkmnopqrstuvwxyz" : "abcdefghijklmnopqrstuvwxyz";
  const numbers = excludeSimilar ? "23456789" : "0123456789";
  const symbols = "!@#$%^&*()-_=+[]{};:,.<>?";

  let alphabet = "";
  if (includeUpper) alphabet += upper;
  if (includeLower) alphabet += lower;
  if (includeNumbers) alphabet += numbers;
  if (includeSymbols) alphabet += symbols;
  if (!alphabet) {
    throw new Error("At least one character category must be enabled.");
  }

  let out = "";
  for (let i = 0; i < length; i += 1) {
    out += alphabet[randomInt(0, alphabet.length)];
  }
  return out;
}

export interface RegexTestResult {
  index: number;
  match: string;
  groups: string[];
}

export function regexTest(pattern: string, flags: string, input: string): RegexTestResult[] {
  const safeFlags = flags.includes("g") ? flags : `${flags}g`;
  const regex = new RegExp(pattern, safeFlags);
  const results: RegexTestResult[] = [];
  for (const match of input.matchAll(regex)) {
    results.push({
      index: match.index ?? 0,
      match: match[0],
      groups: match.slice(1)
    });
  }
  return results;
}

export function generateRegex(kind: "email" | "ipv4" | "url" | "phone-cn"): string {
  const templates: Record<string, string> = {
    email: "^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$",
    ipv4: "^(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)(\\.(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)){3}$",
    url: "^(https?:\\/\\/)?([\\w-]+\\.)+[\\w-]+([\\w\\-./?%&=]*)?$",
    "phone-cn": "^1[3-9]\\d{9}$"
  };
  return templates[kind] ?? templates.email;
}

export function previewCronNextRuns(expression: string, count = 5): string[] {
  const interval = CronExpressionParser.parse(expression);
  const out: string[] = [];
  for (let i = 0; i < count; i += 1) {
    out.push(interval.next().toString());
  }
  return out;
}

export interface CronFields {
  second?: string;
  minute?: string;
  hour?: string;
  dayOfMonth?: string;
  month?: string;
  dayOfWeek?: string;
}

export function generateCronExpression(fields: CronFields): string {
  const second = fields.second?.trim() || "0";
  const minute = fields.minute?.trim() || "*";
  const hour = fields.hour?.trim() || "*";
  const dayOfMonth = fields.dayOfMonth?.trim() || "*";
  const month = fields.month?.trim() || "*";
  const dayOfWeek = fields.dayOfWeek?.trim() || "*";
  return `${second} ${minute} ${hour} ${dayOfMonth} ${month} ${dayOfWeek}`;
}

export async function generateQrCodeDataUrl(text: string): Promise<string> {
  const svg = await QRCode.toString(text, {
    type: "svg",
    errorCorrectionLevel: "M",
    margin: 2
  });
  const encoded = Buffer.from(svg, "utf8").toString("base64");
  return `data:image/svg+xml;base64,${encoded}`;
}
