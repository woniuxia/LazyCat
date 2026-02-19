/**
 * 用 Puppeteer 抓取 Element Plus 中文文档
 * 用法: node scripts/scrape-element-plus.mjs
 */
import puppeteer from "puppeteer";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT = path.resolve(__dirname, "../resources/manuals/element-plus");

const BASE = "https://element-plus.org";
const SITEMAP = `${BASE}/sitemap.xml`;

async function main() {
  // 1. 获取 sitemap 中所有 zh-CN 页面
  console.log("获取 sitemap...");
  const res = await fetch(SITEMAP);
  const xml = await res.text();
  const urls = [...xml.matchAll(/<loc>(.*?)<\/loc>/g)]
    .map((m) => m[1])
    .filter((u) => u.includes("/zh-CN/"));
  console.log(`找到 ${urls.length} 个中文页面`);

  // 2. 启动浏览器
  const browser = await puppeteer.launch({
    headless: true,
    executablePath: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 800 });

  // 收集所有静态资源 URL
  const assetUrls = new Set();

  // 3. 逐页抓取
  let count = 0;
  for (const url of urls) {
    count++;
    const relPath = new URL(url).pathname; // e.g. /zh-CN/guide/design.html
    const filePath = path.join(OUTPUT, relPath.endsWith("/") ? relPath + "index.html" : relPath);

    if (fs.existsSync(filePath)) {
      console.log(`[${count}/${urls.length}] 跳过 ${relPath}`);
      continue;
    }

    console.log(`[${count}/${urls.length}] ${relPath}`);
    try {
      const response = await page.goto(url, { waitUntil: "networkidle0", timeout: 30000 });
      if (!response || !response.ok()) {
        console.log(`  跳过 (HTTP ${response?.status()})`);
        continue;
      }
      // 等待内容渲染
      await page.waitForSelector("#app .VPContent", { timeout: 10000 }).catch(() => {});

      // 收集该页面的资源
      const pageAssets = await page.evaluate(() => {
        const assets = [];
        document.querySelectorAll('link[rel="stylesheet"], link[rel="preload"][as="style"]').forEach((el) => {
          if (el.href) assets.push(el.href);
        });
        document.querySelectorAll("script[src]").forEach((el) => {
          if (el.src) assets.push(el.src);
        });
        document.querySelectorAll('link[rel="preload"][as="font"]').forEach((el) => {
          if (el.href) assets.push(el.href);
        });
        document.querySelectorAll('link[rel="modulepreload"]').forEach((el) => {
          if (el.href) assets.push(el.href);
        });
        document.querySelectorAll("img[src]").forEach((el) => {
          if (el.src && !el.src.startsWith("data:")) assets.push(el.src);
        });
        return assets;
      });
      for (const a of pageAssets) {
        if (a.startsWith(BASE)) assetUrls.add(a);
      }

      // 获取渲染后的 HTML
      let html = await page.content();

      // 将绝对 URL 转为相对路径
      html = html.replaceAll(BASE, "");

      fs.mkdirSync(path.dirname(filePath), { recursive: true });
      fs.writeFileSync(filePath, html, "utf-8");
    } catch (err) {
      console.log(`  失败: ${err.message}`);
    }
  }

  // 4. 下载静态资源（CSS/JS/字体/图片）
  console.log(`\n下载 ${assetUrls.size} 个静态资源...`);
  let assetCount = 0;
  for (const url of assetUrls) {
    assetCount++;
    const relPath = new URL(url).pathname;
    const filePath = path.join(OUTPUT, relPath);
    if (fs.existsSync(filePath)) continue;

    try {
      const resp = await fetch(url);
      if (!resp.ok) continue;
      const buf = Buffer.from(await resp.arrayBuffer());
      fs.mkdirSync(path.dirname(filePath), { recursive: true });
      fs.writeFileSync(filePath, buf);
      if (assetCount % 50 === 0) console.log(`  ${assetCount}/${assetUrls.size}`);
    } catch {
      // 跳过
    }
  }

  // 5. 下载 vp-icons.css（根路径资源）
  for (const rootAsset of ["/vp-icons.css", "/images/element-plus-logo-small.svg"]) {
    const filePath = path.join(OUTPUT, rootAsset);
    if (!fs.existsSync(filePath)) {
      try {
        const resp = await fetch(`${BASE}${rootAsset}`);
        if (resp.ok) {
          const buf = Buffer.from(await resp.arrayBuffer());
          fs.mkdirSync(path.dirname(filePath), { recursive: true });
          fs.writeFileSync(filePath, buf);
        }
      } catch {}
    }
  }

  await browser.close();
  console.log(`\n完成! 输出目录: ${OUTPUT}`);
}

main().catch(console.error);
