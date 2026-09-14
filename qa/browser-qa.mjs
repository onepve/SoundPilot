/* 浏览器交互 QA（开发测试用，不打包）：
 * - 加载 dev server
 * - 桌面两档分辨率截图
 * - 三页切换、离线横幅、规则 CRUD 交互冒烟
 * - 控制台错误收集
 */
import { chromium } from 'playwright-core'
import { mkdirSync } from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'
const BASE = process.env.BASE_URL || 'http://localhost:1420'

const OUT = new URL('../qa-artifacts', import.meta.url).pathname
mkdirSync(OUT, { recursive: true })

const EXE = process.env.CHROMIUM_PATH
  || `${process.env.HOME}/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`
const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })

const results = { consoleErrors: [], checks: [] }
function check(name, ok, extra = '') {
  results.checks.push(`${ok ? 'PASS' : 'FAIL'} ${name}${extra ? ' — ' + extra : ''}`)
  if (!ok) process.exitCode = 1
}

async function runViewport(w, h) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h } })
  const page = await ctx.newPage()
  page.on('console', (m) => {
    if (m.type() === 'error') results.consoleErrors.push(`[${w}x${h}] ${m.text()}`)
  })
  page.on('pageerror', (e) => results.consoleErrors.push(`[${w}x${h}] pageerror: ${e.message}`))

  await page.goto(`${BASE}/?demo=1`, { waitUntil: 'networkidle' })
  await sleep(400)

  // 1) 离线横幅可见（普通浏览器无桥）
  const banner = await page.getByText('演示模式（开发测试用）', { exact: false }).count()
  check(`${w}x${h} 离线横幅`, banner > 0)

  // 首页
  await page.screenshot({ path: `${OUT}/home-${w}x${h}.png` })

  // 首页文案：手动音量说明"先暂停"
  const manualNote = await page.locator('text=先暂停自动监控').count()
  check(`${w}x${h} 手动音量先暂停说明`, manualNote > 0)

  // 2) 规则页：新建规则弹窗
  await page.locator('aside').getByRole('button', { name: /游戏与平台/ }).click()
  await sleep(300)
  await page.screenshot({ path: `${OUT}/rules-empty-${w}x${h}.png` })
  await page.getByRole('button', { name: '新建规则' }).click()
  await sleep(300)
  await page.screenshot({ path: `${OUT}/rules-newmodal-${w}x${h}.png` })

  // 空表单：保存按钮禁用（校验拦截）
  const saveBtn = page.getByRole('button', { name: /创建规则/ })
  check(`${w}x${h} 空表单禁用保存`, await saveBtn.isDisabled())

  // 只填进程不填名称 -> 仍禁用；补名称 -> 启用
  await page.getByPlaceholder(/进程名/).first().fill('TestGame')
  await sleep(150)
  check(`${w}x${h} 缺名称仍禁用`, await saveBtn.isDisabled())
  await page.getByPlaceholder('例如：英雄联盟').fill('测试游戏')
  await sleep(150)
  check(`${w}x${h} 表单齐全启用保存`, await saveBtn.isEnabled())

  await saveBtn.click()
  await sleep(300)
  const card = await page.locator('article', { hasText: '测试游戏' }).count()
  check(`${w}x${h} 规则卡片出现`, card === 1)

  // 3) 复制
  await page.locator('article', { hasText: '测试游戏' }).getByRole('button', { name: '复制' }).click()
  await sleep(300)
  const copyCard = await page.locator('article', { hasText: '测试游戏 (副本)' }).count()
  check(`${w}x${h} 复制规则`, copyCard === 1)

  // 上移下移
  await page.locator('article', { hasText: '测试游戏 (副本)' }).getByRole('button', { name: '上移' }).click()
  await sleep(250)
  const firstIsCopy = await page.locator('article').nth(2).textContent()
  check(`${w}x${h} 上移交换顺序`, (firstIsCopy ?? '').includes('副本'))

  // 4) 删除确认弹窗
  await page.locator('article', { hasText: '测试游戏 (副本)' }).getByRole('button', { name: '删除' }).click()
  await sleep(300)
  const confirmVisible = await page.locator('text=删除规则').count()
  check(`${w}x${h} 删除确认弹窗`, confirmVisible > 0)
  await page.screenshot({ path: `${OUT}/rules-confirm-${w}x${h}.png` })
  await page.locator('.fixed.inset-0').getByRole('button', { name: '删除', exact: true }).click()
  await sleep(300)
  const afterDel = await page.locator('article', { hasText: '测试游戏 (副本)' }).count()
  check(`${w}x${h} 确认后删除`, afterDel === 0)

  // 5) 搜索过滤
  await page.getByPlaceholder('搜索规则名称或进程…').fill('不存在的东西')
  await sleep(300)
  const empty = await page.locator('text=没有匹配的规则').count()
  check(`${w}x${h} 搜索空态`, empty > 0)
  await page.getByPlaceholder('搜索规则名称或进程…').fill('')

  // 6) 平台模板
  await page.getByRole('button', { name: /平台模板/ }).click()
  await sleep(300)
  await page.screenshot({ path: `${OUT}/rules-template-${w}x${h}.png` })
  const tmplNote = await page.locator('text=并不检测本机是否已安装').count()
  check(`${w}x${h} 模板免责说明`, tmplNote > 0)
  // 关闭模板弹窗：点击右上角关闭
  await page.locator('.fixed.inset-0').getByRole('button', { name: '关闭' }).click()
  await sleep(250)
  const tmplGone = await page.locator('text=平台模板').count()
  check(`${w}x${h} 模板弹窗可关闭`, (await page.locator('.fixed.inset-0').count()) === 0)

  // 7) 设置页
  await page.locator('aside').getByRole('button', { name: '设置' }).click()
  await sleep(400)
  await page.screenshot({ path: `${OUT}/settings-${w}x${h}.png` })
  const serverField = await page.getByPlaceholder('http://192.168.1.10:8080').count()
  check(`${w}x${h} 设置页服务连接`, serverField > 0)

  // 添加音箱交互
  await page.getByPlaceholder('例如 34939a12...').fill('did-qa-1')
  await page.getByPlaceholder('卧室').fill('QA音箱')
  await page.getByRole('button', { name: '添加' }).click()
  await sleep(250)
  const spk = await page.locator('text=did-qa-1').count()
  check(`${w}x${h} 添加音箱`, spk > 0)

  // 8) 首页（回到）状态卡在离线下的表现
  await page.locator('aside').getByRole('button', { name: '首页' }).click()
  await sleep(300)
  const demoHome = await page.locator('text=演示模式 · 未连接').count()
  check(`${w}x${h} 首页演示状态卡`, demoHome > 0)

  // 水平溢出检测
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  check(`${w}x${h} 无水平溢出`, overflow <= 0, `diff=${overflow}px`)

  await ctx.close()
}

await runViewport(1120, 760)
await runViewport(900, 650)

await browser.close()

console.log(results.checks.join('\n'))
console.log(`\nconsole errors: ${results.consoleErrors.length}`)
for (const e of results.consoleErrors) console.log('  ' + e)
if (results.consoleErrors.length) process.exitCode = 1
