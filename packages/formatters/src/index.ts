import { format } from "prettier/standalone";
import parserBabel from "prettier/plugins/babel";
import parserEstree from "prettier/plugins/estree";
import parserHtml from "prettier/plugins/html";
import { format as formatSql } from "sql-formatter";
import formatXmlLib from "xml-formatter";

export async function formatJson(input: string): Promise<string> {
  return format(input, { parser: "json", plugins: [parserBabel, parserEstree] });
}

export async function formatHtml(input: string): Promise<string> {
  return format(input, { parser: "html", plugins: [parserHtml] });
}

export function formatXml(input: string): string {
  return formatXmlLib(input, { collapseContent: true, lineSeparator: "\n" });
}

export function formatSqlCode(input: string): string {
  return formatSql(input, { language: "sql" });
}

export function formatJava(input: string): string {
  // V1 uses a conservative fallback to avoid corrupting source code semantics.
  return input
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .join("\n");
}
