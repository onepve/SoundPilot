/* 日志卡片截图（整页 + 精确定位卡片元素） */
import { chromium } from 'playwright-core'
import { setTimeout as sleep } from 'node:timers/promises'
const BASE = process.env.BASE_URL || 'http://localhost:1420'

const EXE =
  process.env.CHROMIUM_PATH ||
  [process.env.HOME, '.cache/ms-playwright/chromium-1243/chrome-linux64/chrome'].join('/')

const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
const page = await browser.newPage({ viewport: { width: 1120, height: 760 } })
await page.goto(`${BASE}/?demo=1`, { waitUntil: 'networkidle' })
await sleep(800)

const card = page.locator('section', { hasText: '运行日志' }).first()
await card.screenshot({ path: 'qa-artifacts/logs-card-full.png' })
const box = await card.boundingBox()
console.log('日志卡片区域:', JSON.stringify(box))
console.log('标题文本:', (await card.locator('h3').innerText()).trim())
console.log('徽标:', (await card.getByText(/最多保留/).innerText()).trim())
console.log('按钮:', (await card.getByRole('button', { name: /清空日志/ }).innerText()).trim())

// 清空后再截一张
await card.getByRole('button', { name: /清空日志/ }).click()
await sleep(400)
await page.locator('.fixed.inset-0').getByRole('button', { name: '清空' }).click()
await sleep(700)
await card.screenshot({ path: 'qa-artifacts/logs-card-empty.png' })
console.log('清空后卡片文本:', (await card.innerText()).replace(/\n/g, ' | '))
await page.screenshot({ path: 'qa-artifacts/home-after-clear.png' })
await browser.close()
