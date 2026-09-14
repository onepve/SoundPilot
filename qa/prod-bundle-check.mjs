/* 生产构建产物（dist/）真实渲染验证。
 * 与 dev server 不同，生产包会走 esbuild 压缩、tree-shaking 与 Tauri 资源内嵌路径，
 * 是 Windows 原生窗口实际加载的那份代码；白屏问题必须在这一层排查。
 */
import { chromium } from 'playwright-core'
import { createServer } from 'node:http'
import { readFile, stat } from 'node:fs/promises'
import { extname, join } from 'node:path'
import { fileURLToPath, URL } from 'node:url'

const DIST = fileURLToPath(new URL('../dist', import.meta.url))
const PORT = 4299

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
}

const server = createServer(async (req, res) => {
  try {
    const url = (req.url || '/').split('?')[0]
    let path = join(DIST, url === '/' ? 'index.html' : url)
    const st = await stat(path).catch(() => null)
    if (!st || st.isDirectory()) path = join(DIST, 'index.html')
    const body = await readFile(path)
    res.writeHead(200, { 'content-type': MIME[extname(path)] || 'application/octet-stream' })
    res.end(body)
  } catch (e) {
    res.writeHead(404).end('not found')
  }
})
await new Promise((r) => server.listen(PORT, r))

// 本机 Chromium 路径：优先环境变量，其次 HOME 下的 playwright 缓存
const EXE = process.env.CHROMIUM_PATH
  || `${process.env.HOME}/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`
const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
const page = await browser.newPage({ viewport: { width: 1120, height: 760 } })

const errors = []
const failed = []
page.on('pageerror', (e) => errors.push(String(e.message || e)))
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`)
})
page.on('requestfailed', (r) => failed.push(`${r.url()} :: ${r.failure()?.errorText}`))

let failedCount = 0
const check = (name, ok, extra = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${extra ? ' — ' + extra : ''}`)
  if (!ok) failedCount++
}

// 生产构建：无 ?demo=1，因此应显示离线横幅并给出明确说明（不伪造桌面能力）
await page.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' })
await page.waitForTimeout(1500)

const rootHtmlLen = (await page.locator('#app').innerHTML()).length
check('生产包挂载出内容（非白屏）', rootHtmlLen > 500, `#app 内容长度=${rootHtmlLen}`)

const bodyText = await page.locator('body').innerText().catch(() => '')
check('渲染出中文界面文字', bodyText.includes('首页') || bodyText.includes('监控'), `文本长度=${bodyText.length}`)

const navCount = await page.locator('aside button').count()
check('左侧导航已渲染', navCount >= 3, `按钮数=${navCount}`)

const offline = await page.getByText(/桌面功能不可用|离线/).count()
check('无桥时明确提示桌面功能不可用', offline > 0, `命中=${offline}`)

// 三页都能渲染
for (const [label, probe] of [
  ['首页', /运行状态|监控/],
  ['游戏与平台', /规则/],
  ['设置', /服务连接|音箱|设置/],
]) {
  await page.locator('aside').getByRole('button', { name: new RegExp(label) }).first().click()
  await page.waitForTimeout(600)
  const t = await page.locator('main').innerText().catch(() => '')
  check(`生产包「${label}」页可渲染`, probe.test(t), `main 文本=${t.length}`)
}

await page.screenshot({ path: 'qa-artifacts/prod-bundle-home.png' })

check('无 JS 运行时错误', errors.length === 0, errors.slice(0, 3).join(' | '))
check('无资源加载失败', failed.length === 0, failed.slice(0, 3).join(' | '))

console.log(`errors=${errors.length} requestfailed=${failed.length}`)
await browser.close()
server.close()
process.exit(failedCount ? 1 : 0)
