import DOMPurify from "dompurify";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import MarkdownIt from "markdown-it";
import taskLists from "markdown-it-task-lists";
import "highlight.js/styles/github.css";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("css", css);
hljs.registerLanguage("java", java);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);
hljs.registerLanguage("python", python);
hljs.registerLanguage("py", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

const escapeHtml = (text: string): string =>
  text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");

const markdownRenderer: MarkdownIt = new MarkdownIt({
  breaks: false,
  html: false,
  linkify: true,
  typographer: false,
  highlight(code: string, language: string): string {
    const normalizedLanguage = language.trim().toLowerCase();
    if (normalizedLanguage && hljs.getLanguage(normalizedLanguage)) {
      return hljs.highlight(code, { language: normalizedLanguage }).value;
    }
    return escapeHtml(code);
  },
}).use(taskLists, { enabled: false, label: true, labelAfter: true });

const linkOpenRule: NonNullable<typeof markdownRenderer.renderer.rules.link_open> = (
  tokens,
  index,
  options,
  _env,
  self,
) => {
  tokens[index].attrSet("target", "_blank");
  tokens[index].attrSet("rel", "noopener noreferrer");
  return self.renderToken(tokens, index, options);
};
markdownRenderer.renderer.rules.link_open = linkOpenRule;

type Sanitizer = { sanitize?: (html: string, options?: Record<string, unknown>) => string };

export function renderMarkdown(text: string): string {
  const html = markdownRenderer.render(text);
  const sanitizer = DOMPurify as unknown as Sanitizer;
  if (typeof sanitizer.sanitize !== "function") return html;
  return sanitizer.sanitize(html, {
    USE_PROFILES: { html: true },
    ADD_ATTR: ["target"],
  });
}
