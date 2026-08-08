export type ExceptionStackFormat = "javascript" | "java";
export type ExceptionStackFormatOverride = "auto" | ExceptionStackFormat;
export type ExceptionStackDetection = ExceptionStackFormat | "ambiguous" | "unsupported";

export interface ExceptionStackException {
  type: string;
  message: string;
  lineNumber: number;
}

export interface ExceptionStackFrame {
  format: ExceptionStackFormat;
  functionName: string;
  filePath: string;
  line: number | null;
  column: number | null;
  lineNumber: number;
  raw: string;
}

export interface ExceptionStackOmissionMarker {
  count: number;
  lineNumber: number;
  raw: string;
}

export interface ExceptionStackUnrecognizedLine {
  lineNumber: number;
  text: string;
}

export interface ExceptionStackResult {
  ok: boolean;
  format: ExceptionStackFormat | null;
  formatSource: "auto" | "manual";
  detection: ExceptionStackDetection;
  rootException: ExceptionStackException | null;
  causes: ExceptionStackException[];
  frames: ExceptionStackFrame[];
  omittedFrameCount: number;
  abbreviatedFrameCount: number;
  omissionMarkers: ExceptionStackOmissionMarker[];
  unrecognizedLines: ExceptionStackUnrecognizedLine[];
  diagnostics: string[];
  summary: string;
}

const MAX_SUMMARY_FRAMES = 5;
const IDENTIFIER = String.raw`[A-Za-z_$][\w$]*`;
const JAVASCRIPT_ERROR_SUFFIX_SOURCE = "(?:Error|Exception|Failure|Rejection|Problem)";
const JAVA_ERROR_SUFFIX_SOURCE = "(?:Exception|Error|Throwable|Failure|Problem)";
const JAVA_ERROR_SUFFIX = /(?:Exception|Error|Throwable|Failure|Problem)$/;
const KNOWN_JAVASCRIPT_TYPES = new Set([
  "AbortError",
  "DOMException",
  "Error",
  "EvalError",
  "RangeError",
  "ReferenceError",
  "SyntaxError",
  "TypeError",
  "URIError",
]);

type ParsedHeader = ExceptionStackException;
type LocationParts = {
  functionName: string;
  filePath: string;
  line: number;
  column: number | null;
};

function emptyResult(
  format: ExceptionStackFormat | null,
  formatSource: "auto" | "manual",
  detection: ExceptionStackDetection,
  diagnostics: string[],
  unrecognizedLines: ExceptionStackUnrecognizedLine[] = [],
): ExceptionStackResult {
  return {
    ok: false,
    format,
    formatSource,
    detection,
    rootException: null,
    causes: [],
    frames: [],
    omittedFrameCount: 0,
    abbreviatedFrameCount: 0,
    omissionMarkers: [],
    unrecognizedLines,
    diagnostics,
    summary: "",
  };
}

function nonEmptyLines(rawText: string): ExceptionStackUnrecognizedLine[] {
  return rawText.split(/\r?\n/).flatMap((text, index) => {
    if (!text.trim()) return [];
    return [{ lineNumber: index + 1, text }];
  });
}

function parseJavaScriptHeader(line: string, lineNumber: number): ParsedHeader | null {
  let text = line.trim();
  text = text.replace(/^Uncaught(?:\s+\(in promise\))?\s+/, "");

  const match = text.match(
    new RegExp(
      `^((?:${IDENTIFIER}\.)*[A-Za-z_$]*?${JAVASCRIPT_ERROR_SUFFIX_SOURCE}(?:\\s+\\[[^\\]]+\\])?)(?::\\s*(.*))?$`,
    ),
  );
  if (!match) return null;

  return {
    type: match[1],
    message: match[2] ?? "",
    lineNumber,
  };
}

function parseJavaHeader(line: string, lineNumber: number): ParsedHeader | null {
  let text = line.trim();
  const threadMatch = text.match(/^Exception in thread\s+"[^"]+"\s+(.+)$/);
  if (threadMatch) text = threadMatch[1];
  if (text.startsWith("Caused by:")) text = text.slice("Caused by:".length).trim();

  const match = text.match(
    new RegExp(`^((?:${IDENTIFIER}\.)*[A-Za-z_$]*${JAVA_ERROR_SUFFIX_SOURCE})(?::\\s*(.*))?$`),
  );
  if (!match) return null;

  return {
    type: match[1],
    message: match[2] ?? "",
    lineNumber,
  };
}

function isJavaHeaderCandidate(header: ParsedHeader): boolean {
  const baseType = header.type.split(/\s+\[/)[0] ?? header.type;
  if (KNOWN_JAVASCRIPT_TYPES.has(baseType)) return false;
  if (baseType === "Error" && !baseType.includes(".")) return false;
  return baseType.includes(".") || /(?:Exception|Throwable|Failure|Problem)$/.test(baseType);
}

function parseJavaFrame(line: string, lineNumber: number): ExceptionStackFrame | null {
  const match = line.trim().match(/^at\s+(.+?)\((.*)\)$/);
  if (!match || !match[1].includes(".")) return null;

  const functionName = match[1].trim();
  const location = match[2].trim();
  if (location === "Native Method" || location === "Unknown Source") {
    return {
      format: "java",
      functionName,
      filePath: location,
      line: null,
      column: null,
      lineNumber,
      raw: line,
    };
  }

  const locationMatch = location.match(/^(.+\.java):(\d+)$/i);
  if (!locationMatch) return null;

  return {
    format: "java",
    functionName,
    filePath: locationMatch[1],
    line: Number(locationMatch[2]),
    column: null,
    lineNumber,
    raw: line,
  };
}

function parseLocationSuffix(text: string): LocationParts | null {
  let value = text.trim();
  while (value.endsWith(")")) value = value.slice(0, -1).trim();

  const match = value.match(/^(.*?):(\d+)(?::(\d+))?$/);
  if (!match) return null;

  const prefix = match[1].trim();
  const openParen = prefix.lastIndexOf("(");
  if (openParen >= 0) {
    const filePath = prefix.slice(openParen + 1).trim();
    if (!filePath) return null;
    return {
      functionName: prefix.slice(0, openParen).trim(),
      filePath,
      line: Number(match[2]),
      column: match[3] ? Number(match[3]) : null,
    };
  }

  const atIndex = prefix.indexOf("@");
  if (atIndex >= 0) {
    const functionName = prefix.slice(0, atIndex).trim();
    const filePath = prefix.slice(atIndex + 1).trim();
    if (filePath && (!functionName || !/[\\/:]/.test(functionName))) {
      return {
        functionName,
        filePath,
        line: Number(match[2]),
        column: match[3] ? Number(match[3]) : null,
      };
    }
  }

  if (!prefix) return null;
  const asyncPath = prefix.match(/^async\s+((?:[A-Za-z][\w+.-]*:\/\/|[A-Za-z]:[\\/]|[\\/]).+)$/);
  return {
    functionName: "",
    filePath: asyncPath?.[1] ?? prefix,
    line: Number(match[2]),
    column: match[3] ? Number(match[3]) : null,
  };
}

function parseJavaScriptFrame(line: string, lineNumber: number): ExceptionStackFrame | null {
  const text = line.trim();
  const looksLikeFrame =
    text.startsWith("at ") || text.includes("@") || /(?:https?|file|blob|webpack):\/\//.test(text);
  if (!looksLikeFrame) return null;

  const javaFrame = parseJavaFrame(line, lineNumber);
  if (javaFrame && !/\d+:\d+\)?$/.test(text)) return null;

  const body = text.startsWith("at ") ? text.slice(3) : text;
  const location = parseLocationSuffix(body);
  if (!location) return null;

  return {
    format: "javascript",
    ...location,
    lineNumber,
    raw: line,
  };
}

function parseJavaOmissionMarker(
  line: string,
  lineNumber: number,
): ExceptionStackOmissionMarker | null {
  const match = line.trim().match(/^\.\.\.\s+(\d+)\s+more$/);
  if (!match) return null;
  return { count: Number(match[1]), lineNumber, raw: line };
}

function scoreFormat(rawText: string): {
  format: ExceptionStackFormat | null;
  detection: ExceptionStackDetection;
} {
  const lines = rawText.split(/\r?\n/);
  const javascriptHeaders = lines.flatMap((line, index) =>
    parseJavaScriptHeader(line, index + 1) ? [1] : [],
  ).length;
  const javaHeaders = lines.flatMap((line, index) => {
    const header = parseJavaHeader(line, index + 1);
    return header && isJavaHeaderCandidate(header) ? [header] : [];
  });
  const javascriptFrames = lines.flatMap((line, index) =>
    parseJavaScriptFrame(line, index + 1) ? [1] : [],
  ).length;
  const javaFrames = lines.flatMap((line, index) =>
    parseJavaFrame(line, index + 1) ? [1] : [],
  ).length;
  const causeCount = lines.filter((line) => line.trim().startsWith("Caused by:")).length;
  const omissionCount = lines.flatMap((line, index) =>
    parseJavaOmissionMarker(line, index + 1) ? [1] : [],
  ).length;

  const javascriptScore = javascriptHeaders * 2 + javascriptFrames * 3;
  const javaHeaderScore = javaHeaders.reduce(
    (score, header) => score + (header.type.includes(".") ? 3 : 2),
    0,
  );
  const javaScore = javaHeaderScore + javaFrames * 4 + causeCount * 5 + omissionCount * 5;

  if (javascriptScore === 0 && javaScore === 0) {
    return { format: null, detection: "unsupported" };
  }
  if (javascriptScore === javaScore) {
    return { format: null, detection: "ambiguous" };
  }
  if (javascriptScore > javaScore) {
    return { format: "javascript", detection: "javascript" };
  }
  return { format: "java", detection: "java" };
}

function formatException(exception: ExceptionStackException): string {
  return exception.message ? `${exception.type}: ${exception.message}` : exception.type;
}

function formatFrame(frame: ExceptionStackFrame): string {
  const location = `${frame.filePath}${frame.line === null ? "" : `:${frame.line}`}${
    frame.column === null ? "" : `:${frame.column}`
  }`;
  if (frame.format === "java") return `at ${frame.functionName}(${location})`;
  return frame.functionName ? `at ${frame.functionName} (${location})` : `at ${location}`;
}

function buildSummary(
  format: ExceptionStackFormat,
  rootException: ExceptionStackException | null,
  causes: ExceptionStackException[],
  frames: ExceptionStackFrame[],
  omittedFrameCount: number,
  abbreviatedFrameCount: number,
  unrecognizedLines: ExceptionStackUnrecognizedLine[],
): string {
  const summary = [
    `格式: ${format === "java" ? "Java" : "JavaScript/TypeScript"}`,
    `异常: ${rootException ? formatException(rootException) : "未识别"}`,
  ];

  if (causes.length > 0) {
    summary.push("原因链:");
    summary.push(...causes.map((cause) => `- ${formatException(cause)}`));
  }

  summary.push("调用帧:");
  summary.push(
    ...(frames.length > 0 ? frames.map((frame) => `- ${formatFrame(frame)}`) : ["- 无"]),
  );
  if (omittedFrameCount > 0) summary.push(`省略 ${omittedFrameCount} 个可识别帧`);
  if (abbreviatedFrameCount > 0) {
    summary.push(`Java 公共帧标记省略 ${abbreviatedFrameCount} 个帧`);
  }
  if (unrecognizedLines.length > 0) {
    summary.push(`未识别行: ${unrecognizedLines.length}`);
  }
  return summary.join("\n");
}

function addDiagnostic(diagnostics: string[], message: string): void {
  if (!diagnostics.includes(message)) diagnostics.push(message);
}

function parseWithFormat(
  rawText: string,
  format: ExceptionStackFormat,
  formatSource: "auto" | "manual",
  detection: ExceptionStackDetection,
): ExceptionStackResult {
  const lines = rawText.split(/\r?\n/);
  const frames: ExceptionStackFrame[] = [];
  const causes: ExceptionStackException[] = [];
  const omissionMarkers: ExceptionStackOmissionMarker[] = [];
  const unrecognizedLines: ExceptionStackUnrecognizedLine[] = [];
  const diagnostics: string[] = [];
  let rootException: ExceptionStackException | null = null;
  let multipleRoots = false;

  lines.forEach((line, index) => {
    const lineNumber = index + 1;
    if (!line.trim()) return;

    const isCause = format === "java" && line.trim().startsWith("Caused by:");
    const header =
      format === "java"
        ? parseJavaHeader(line, lineNumber)
        : parseJavaScriptHeader(line, lineNumber);
    if (header) {
      if (isCause) {
        if (!rootException) {
          addDiagnostic(diagnostics, "原因链出现在根异常之前，无法建立完整的异常链");
        } else {
          causes.push(header);
        }
      } else if (rootException) {
        multipleRoots = true;
        addDiagnostic(diagnostics, "输入包含多个独立的根异常，请拆分后分别解析");
      } else {
        rootException = header;
      }
      return;
    }

    if (format === "java") {
      const omissionMarker = parseJavaOmissionMarker(line, lineNumber);
      if (omissionMarker) {
        omissionMarkers.push(omissionMarker);
        return;
      }
    }

    const frame =
      format === "java" ? parseJavaFrame(line, lineNumber) : parseJavaScriptFrame(line, lineNumber);
    if (frame) {
      frames.push(frame);
      return;
    }

    unrecognizedLines.push({ lineNumber, text: line });
  });

  const omittedFrameCount = Math.max(0, frames.length - MAX_SUMMARY_FRAMES);
  const selectedFrames = omittedFrameCount > 0 ? frames.slice(-MAX_SUMMARY_FRAMES) : frames;
  const abbreviatedFrameCount = omissionMarkers.reduce((total, marker) => total + marker.count, 0);

  if (unrecognizedLines.length > 0) {
    addDiagnostic(diagnostics, `有 ${unrecognizedLines.length} 行未识别，原文已保留`);
  }
  if (!rootException && frames.length === 0) {
    addDiagnostic(diagnostics, "未识别到异常头或调用帧，无法生成摘要");
  }
  if (rootException && frames.length === 0) {
    addDiagnostic(diagnostics, "未识别到调用帧，当前结果仅包含异常信息");
  }

  const ok = !multipleRoots && Boolean(rootException || frames.length > 0);
  if (!ok) {
    return emptyResult(format, formatSource, detection, diagnostics, unrecognizedLines);
  }

  return {
    ok,
    format,
    formatSource,
    detection,
    rootException,
    causes,
    frames: selectedFrames,
    omittedFrameCount,
    abbreviatedFrameCount,
    omissionMarkers,
    unrecognizedLines,
    diagnostics,
    summary: ok
      ? buildSummary(
          format,
          rootException,
          causes,
          selectedFrames,
          omittedFrameCount,
          abbreviatedFrameCount,
          unrecognizedLines,
        )
      : "",
  };
}

export function detectExceptionStackFormat(rawText: string): {
  format: ExceptionStackFormat | null;
  detection: ExceptionStackDetection;
} {
  return scoreFormat(rawText);
}

export function parseExceptionStack(
  rawText: string,
  formatOverride: ExceptionStackFormatOverride = "auto",
): ExceptionStackResult {
  const formatSource = formatOverride === "auto" ? "auto" : "manual";
  let format: ExceptionStackFormat | null;
  let detection: ExceptionStackDetection;
  if (formatOverride === "auto") {
    const detected = scoreFormat(rawText);
    format = detected.format;
    detection = detected.detection;
  } else {
    format = formatOverride;
    detection = formatOverride;
  }

  if (!format) {
    const message =
      detection === "ambiguous"
        ? "无法自动判断堆栈格式，请手动选择 JavaScript/TypeScript 或 Java"
        : "无法识别堆栈格式，仅支持 JavaScript/TypeScript 和 Java";
    return emptyResult(null, formatSource, detection, [message], nonEmptyLines(rawText));
  }

  return parseWithFormat(rawText, format, formatSource, detection);
}
