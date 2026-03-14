export const escapeHtml = (text: string): string =>
  text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");

export const renderInline = (line: string): string => {
  let html = escapeHtml(line);
  html = html.replace(/`([^`]+)`/g, "<code>$1</code>");
  html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  html = html.replace(/\*([^*]+)\*/g, "<em>$1</em>");
  html = html.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>',
  );
  return html;
};

export const renderMarkdown = (text: string): string => {
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  const out: string[] = [];
  let inCodeBlock = false;
  let inList = false;

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (!inCodeBlock) {
        if (inList) {
          out.push("</ul>");
          inList = false;
        }
        out.push("<pre><code>");
        inCodeBlock = true;
      } else {
        out.push("</code></pre>");
        inCodeBlock = false;
      }
      continue;
    }

    if (inCodeBlock) {
      out.push(`${escapeHtml(line)}\n`);
      continue;
    }

    if (/^\s*-\s+/.test(line)) {
      if (!inList) {
        out.push("<ul>");
        inList = true;
      }
      out.push(`<li>${renderInline(line.replace(/^\s*-\s+/, ""))}</li>`);
      continue;
    }

    if (inList) {
      out.push("</ul>");
      inList = false;
    }

    if (/^###\s+/.test(line)) {
      out.push(`<h3>${renderInline(line.replace(/^###\s+/, ""))}</h3>`);
      continue;
    }

    if (/^##\s+/.test(line)) {
      out.push(`<h2>${renderInline(line.replace(/^##\s+/, ""))}</h2>`);
      continue;
    }

    if (/^#\s+/.test(line)) {
      out.push(`<h1>${renderInline(line.replace(/^#\s+/, ""))}</h1>`);
      continue;
    }

    if (line.trim().length === 0) {
      out.push("");
      continue;
    }

    out.push(`<p>${renderInline(line)}</p>`);
  }

  if (inList) {
    out.push("</ul>");
  }

  if (inCodeBlock) {
    out.push("</code></pre>");
  }

  return out.join("\n");
};
