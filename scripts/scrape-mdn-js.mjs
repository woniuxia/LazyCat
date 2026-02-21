/**
 * MDN JavaScript 中文手册离线抓取脚本
 * 使用 Puppeteer + 系统 Edge 浏览器
 * 目标：https://developer.mozilla.org/zh-CN/docs/Web/JavaScript
 * 输出：resources/manuals/mdn-js/
 *
 * 用法: node scripts/scrape-mdn-js.mjs
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const puppeteer = require('../node_modules/puppeteer/lib/cjs/puppeteer/puppeteer.js');

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = path.resolve(__dirname, '../resources/manuals/mdn-js');
const BASE_URL = 'https://developer.mozilla.org';
const JS_PREFIX = '/zh-CN/docs/Web/JavaScript';
const CONCURRENCY = 3;
const DELAY_MS = 600;

function shouldSkip(urlPath) {
  return urlPath.includes('?') || urlPath.includes('#') || urlPath.includes('/contributors.txt');
}

async function fetchPage(page, fullUrl) {
  await page.goto(fullUrl, { waitUntil: 'networkidle0', timeout: 30000 });
  await page.waitForSelector('main', { timeout: 8000 }).catch(() => {});

  const links = await page.evaluate((prefix) => {
    return Array.from(document.querySelectorAll('a[href]'))
      .map(a => {
        const href = a.getAttribute('href');
        if (!href) return null;
        if (href.startsWith(prefix)) return href.split('#')[0].split('?')[0];
        return null;
      })
      .filter(Boolean);
  }, JS_PREFIX);

  const html = await page.content();
  const pageAssets = await page.evaluate(() => {
    const urls = [];
    document.querySelectorAll('link[href], script[src]').forEach(el => {
      const src = el.getAttribute('href') || el.getAttribute('src');
      if (src && (src.includes('/static/') || src.includes('/_next/'))) urls.push(src);
    });
    return urls;
  });

  return { html, links: [...new Set(links)], pageAssets };
}

function urlToFilePath(urlPath, outputDir) {
  const rel = urlPath.replace(/^\//, '');
  // 无扩展名的路径（SPA 路由）保存为目录下的 index.html，避免文件和目录同名冲突
  const hasExt = path.extname(rel) !== '';
  if (hasExt) {
    return path.join(outputDir, rel);
  } else {
    return path.join(outputDir, rel, 'index.html');
  }
}

async function savePage(urlPath, html, outputDir) {
  const filePath = urlToFilePath(urlPath, outputDir);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, html, 'utf-8');
}

async function downloadAsset(assetUrl, outputDir) {
  try {
    const urlObj = new URL(assetUrl.startsWith('http') ? assetUrl : BASE_URL + assetUrl);
    const rel = urlObj.pathname.replace(/^\//, '');
    const filePath = path.join(outputDir, rel);
    if (fs.existsSync(filePath)) return;
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    const res = await fetch(urlObj.toString());
    if (!res.ok) return;
    const buf = await res.arrayBuffer();
    fs.writeFileSync(filePath, Buffer.from(buf));
  } catch {
    // 忽略资源下载失败
  }
}

async function main() {
  console.log('启动 MDN JS 手册抓取...');
  console.log('输出目录:', OUTPUT_DIR);
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  const browser = await puppeteer.default.launch({
    executablePath: 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--lang=zh-CN,zh'],
  });

  const visited = new Set();
  const queue = [JS_PREFIX];
  const assets = new Set();
  let pageCount = 0;

  try {
    while (queue.length > 0) {
      const batch = [];
      while (batch.length < CONCURRENCY && queue.length > 0) {
        const next = queue.shift();
        if (!visited.has(next)) {
          visited.add(next);
          batch.push(next);
        }
      }
      if (batch.length === 0) continue;

      await Promise.all(batch.map(async (urlPath) => {
        const fullUrl = BASE_URL + urlPath;
        console.log(`[${++pageCount}] ${urlPath}`);

        const page = await browser.newPage();
        try {
          const { html, links, pageAssets } = await fetchPage(page, fullUrl);
          await savePage(urlPath, html, OUTPUT_DIR);

          pageAssets.forEach(a => assets.add(a));

          for (const link of links) {
            if (!visited.has(link) && !queue.includes(link) && !shouldSkip(link)) {
              queue.push(link);
            }
          }
        } catch (err) {
          console.warn('  失败:', urlPath, '-', err.message);
        } finally {
          await page.close();
        }

        await new Promise(r => setTimeout(r, DELAY_MS));
      }));
    }

    console.log(`\n页面抓取完成，共 ${pageCount} 页`);
    console.log(`开始下载静态资源 (${assets.size} 个)...`);

    let assetCount = 0;
    for (const assetUrl of assets) {
      await downloadAsset(assetUrl, OUTPUT_DIR);
      if (++assetCount % 20 === 0) console.log(`  资源进度: ${assetCount}/${assets.size}`);
    }

    console.log('\n全部完成！');
    console.log('  页面:', pageCount);
    console.log('  资源:', assetCount);
    console.log('  输出:', OUTPUT_DIR);
  } finally {
    await browser.close();
  }
}

main().catch(err => {
  console.error('抓取失败:', err);
  process.exit(1);
});
