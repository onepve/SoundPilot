/* 日志清空功能 UI 核验：按钮、上限徽标、确认弹窗、清空后状态 */
import { chromium } from 'playwright-core'
import { setTimeout as sleep } from 'node:timers/promises'
const BASE = process.env.BASE_URL || 'http://localhost:1420'

const EXE =
  process.env.CHROMIUM_PATH ||
  [process.env.HOME, '.cache/ms-playwright/chromium-1243/chrome-linux64/chrome'].join('/')

const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
const page = await browser.newPage({ viewport: { width: 1120, height: 760 } })
const errs = []
page.on('pageerror', (e) => errs.push(String(e.message)))
let fail = 0
const check = (n, ok, x = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${n}${x ? ' — ' + x : ''}`)
  if (!ok) fail++
}

await page.goto(`${BASE}/?demo=1`, { waitUntil: 'networkidle' })
await sleep(800)

check('日志卡片显示上限徽标', (await page.getByText('最多保留 200 条').count()) > 0)
const clearBtn = page.getByRole('button', { name: /清空日志/ })
check('存在「清空日志」按钮', (await clearBtn.count()) > 0)

// 演示模式有日志数据，按钮应可用
const disabled = await clearBtn.isDisabled().catch(() => true)
check('有日志时按钮可点击', !disabled)

await clearBtn.click()
await sleep(500)
const modal = await page.locator('.fixed.inset-0').innerText().catch(() => '')
check('弹出二次确认', modal.includes('清空运行日志'), modal.slice(0, 40).replace(/\n/g, ' '))
check('说明日志仅存内存且不影响监控', modal.includes('内存') && modal.includes('监控'))
await page.screenshot({ path: 'qa-artifacts/logs-clear-modal.png' })

// 先取消：日志应保留
await page.locator('.fixed.inset-0').getByRole('button', { name: '取消' }).click()
await sleep(400)
const beforeLines = await page.locator('section', { hasText: '运行日志' }).locator('div.font-mono > div').count()
check('取消后日志保留', beforeLines > 0, `${beforeLines} 行`)

// 再确认清空
await clearBtn.click()
await sleep(400)
await page.locator('.fixed.inset-0').getByRole('button', { name: '清空' }).click()
await sleep(600)
check('确认后弹窗自动关闭', (await page.locator('.fixed.inset-0').count()) === 0)
const afterLines = await page.locator('section', { hasText: '运行日志' }).locator('div.font-mono > div').count()
const emptyHint = await page.getByText('暂无日志').count()
check('确认后日志被清空', afterLines === 0 || emptyHint > 0, `剩 ${afterLines} 行`)
check('清空后按钮变为不可用', await clearBtn.isDisabled())

await page.screenshot({ path: 'qa-artifacts/logs-card-cleared.png', clip: { x: 250, y: 470, width: 850, height: 270 } })
check('无 JS 运行时错误', errs.length === 0, errs.slice(0, 2).join(' | '))

await browser.close()
process.exit(fail ? 1 : 0)
