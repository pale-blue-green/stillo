/**
 * stillo Playwright daemon
 *
 * Unix ソケット上で JSON-lines プロトコルを話す。
 *   request : {"url": "https://..."}\n
 *   response: {"html": "...", "url": "...", "status": 200}\n
 *             {"error": "..."}\n  (失敗時)
 *
 * ブラウザインスタンスは起動時に1つだけ作成し、リクエストごとに新しいページを開く。
 */

'use strict';

const net = require('net');
const fs = require('fs');
const { chromium } = require('playwright');

const SOCKET_PATH = process.env.STILLO_PLAYWRIGHT_SOCK || '/tmp/stillo-playwright.sock';
const NAVIGATION_TIMEOUT_MS = 30_000;

async function handleRequest(browser, socket, line) {
  let req;
  try {
    req = JSON.parse(line);
  } catch (e) {
    socket.write(JSON.stringify({ error: `invalid JSON: ${e.message}` }) + '\n');
    return;
  }

  if (!req.url) {
    socket.write(JSON.stringify({ error: 'missing url field' }) + '\n');
    return;
  }

  const page = await browser.newPage();
  try {
    const response = await page.goto(req.url, {
      waitUntil: 'networkidle',
      timeout: NAVIGATION_TIMEOUT_MS,
    });
    const html = await page.content();
    const finalUrl = page.url();
    const status = response ? response.status() : 200;

    socket.write(JSON.stringify({ html, url: finalUrl, status }) + '\n');
  } catch (e) {
    socket.write(JSON.stringify({ error: e.message }) + '\n');
  } finally {
    await page.close();
  }
}

async function main() {
  if (fs.existsSync(SOCKET_PATH)) {
    fs.unlinkSync(SOCKET_PATH);
  }

  const browser = await chromium.launch({ headless: true });
  console.log('Browser launched');

  const server = net.createServer((socket) => {
    let buf = '';

    socket.on('data', (chunk) => {
      buf += chunk.toString('utf8');

      // 改行ごとにリクエストを処理する（1接続1リクエスト想定）
      let nl;
      while ((nl = buf.indexOf('\n')) !== -1) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (line.length > 0) {
          handleRequest(browser, socket, line).catch((e) => {
            socket.write(JSON.stringify({ error: String(e) }) + '\n');
          });
        }
      }
    });

    socket.on('error', (e) => {
      console.error('socket error:', e.message);
    });
  });

  server.listen(SOCKET_PATH, () => {
    console.log(`Playwright daemon listening on ${SOCKET_PATH}`);
    // 親プロセスへ起動完了を通知する（テスト用）
    if (process.send) process.send('ready');
  });

  process.on('SIGTERM', async () => {
    server.close();
    await browser.close();
    if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH);
    process.exit(0);
  });

  process.on('SIGINT', async () => {
    server.close();
    await browser.close();
    if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH);
    process.exit(0);
  });
}

main().catch((e) => {
  console.error('Failed to start daemon:', e);
  process.exit(1);
});
