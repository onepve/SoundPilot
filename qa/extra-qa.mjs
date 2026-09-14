/* 补充 QA：多进程编辑、音量滑块联动、规则编辑回填 */
import { chromium } from 'playwright-core'
import { setTimeout as sleep } from 'node:timers/promises'
const BASE = process.env.BASE_URL || 'http://localhost:1420'

// 本机 Chromium 路径：优先环境变量，其次 HOME 下的 playwright 缓存
const EXE = process.env.CHROMIUM_PATH
  || `${process.env.HOME}/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`
const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
const ctx = await browser.newContext({ viewport: { width: 1120, height: 760 } })
const page = await ctx.newPage()
const errs = []
page.on('pageerror', (e) => errs.push(e.message))
let fail = 0
const check = (n, ok, x = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${n}${x ? ' — ' + x : ''}`)
  if (!ok) fail++
}

await page.goto(`${BASE}/?demo=1`, { waitUntil: 'networkidle' })
await sleep(400)

// 规则页 -> 编辑演示规则
await page.locator('aside').getByRole('button', { name: /游戏与平台/ }).click()
await sleep(300)
await page.locator('article', { hasText: '英雄联盟（演示）' }).getByRole('button', { name: '编辑' }).click()
await sleep(400)

// 回填检查
const nameVal = await page.getByPlaceholder('例如：英雄联盟').inputValue()
check('编辑回填名称', nameVal === '英雄联盟（演示）', nameVal)
const procCount = await page.locator('input[placeholder*="进程名"]').count()
check('回填多进程（2 条）', procCount === 2, `count=${procCount}`)

// 添加第三个进程行
await page.getByRole('button', { name: /添加进程/ }).click()
await sleep(200)
check('添加进程行', (await page.locator('input[placeholder*="进程名"]').count()) === 3)

// 填第三行 + 保存
await page.locator('input[placeholder*="进程名"]').nth(2).fill('ExtraProc')
await page.locator('.fixed.inset-0').getByRole('button', { name: /保存修改/ }).click()
await sleep(300)
const cardText = await page.locator('article', { hasText: '英雄联盟（演示）' }).textContent()
check('保存后卡片含新进程', (cardText ?? '').includes('ExtraProc'))

// 删除进程行（编辑 -> 移除第三行）
await page.locator('article', { hasText: '英雄联盟（演示）' }).getByRole('button', { name: '编辑' }).click()
await sleep(400)
await page.locator('button[aria-label^="移除进程 3"]').click()
await sleep(200)
check('移除进程行', (await page.locator('input[placeholder*="进程名"]').count()) === 2)
await page.locator('.fixed.inset-0').getByRole('button', { name: /保存修改/ }).click()
await sleep(300)

// 音量滑块联动（编辑弹窗内数字与滑块同步太细节，测设置页滑块联动）
await page.locator('.fixed.inset-0').getByRole('button', { name: '关闭' }).click().catch(() => {})
await sleep(200)
await page.locator('aside').getByRole('button', { name: '设置' }).click()
await sleep(400)
const slider = page.locator('input[type="range"]').first()
const before = await slider.inputValue()
await page.locator('input[type="number"]').first().fill('40')
await sleep(200)
const after = await slider.inputValue()
check('数字输入联动滑块', before !== after, `${before} -> ${after}`)

// 规则启停开关
await page.locator('aside').getByRole('button', { name: /游戏与平台/ }).click()
await sleep(300)
const steamCard = page.locator('article', { hasText: 'Steam（演示）' })
const enabledBefore = (await steamCard.getAttribute('class'))?.includes('opacity-60')
await steamCard.getByRole('switch').click()
await sleep(250)
const enabledAfter = (await steamCard.getAttribute('class'))?.includes('opacity-60')
check('规则启停切换', enabledBefore !== enabledAfter, `${enabledBefore} -> ${enabledAfter}`)

// 分类过滤
await page.locator('aside').getByRole('button', { name: /游戏与平台/ }).click().catch(() => {})
await sleep(200)
const tabs = page.getByRole('button', { name: '平台', exact: true })
await tabs.first().click().catch(async () => {
  // toolbar 分类按钮
  await page.locator('main button', { hasText: '平台' }).first().click()
})
await sleep(300)
const kinds = await page.locator('article').allTextContents()
check('分类过滤只剩平台', kinds.length >= 1 && kinds.every((t) => t.includes('平台') || t.includes('Steam')))

console.log(`pageerrors: ${errs.length}${errs.length ? ' — ' + errs.join('; ') : ''}`)
if (errs.length) fail++

await browser.close()
process.exit(fail ? 1 : 0)
