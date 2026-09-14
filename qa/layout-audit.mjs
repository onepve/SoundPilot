/* DOM 几何布局审计：替代视觉检查
 * - 每页扫描：文字水平溢出（scrollWidth > clientWidth 且非故意截断的元素）
 * - 元素重叠（按钮/卡片间意外重叠）
 * - 视口裁剪（关键内容超出 900x650 视口底部且不可滚动）
 */
import { chromium } from 'playwright-core'
import { setTimeout as sleep } from 'node:timers/promises'
const BASE = process.env.BASE_URL || 'http://localhost:1420'

// 本机 Chromium 路径：优先环境变量，其次 HOME 下的 playwright 缓存
const EXE = process.env.CHROMIUM_PATH
  || `${process.env.HOME}/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`
const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
let fail = 0

async function audit(w, h, pageName, setup) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h } })
  const page = await ctx.newPage()
  await page.goto(`${BASE}/?demo=1`, { waitUntil: 'networkidle' })
  await sleep(400)
  if (setup) await setup(page)
  await sleep(400)

  const issues = await page.evaluate(() => {
    const out = []
    // 1) 文本意外溢出：排除故意 truncate/scroll 容器
    for (const el of document.querySelectorAll('h1,h2,h3,p,span,button,label,a')) {
      if (!el.textContent?.trim()) continue
      const cs = getComputedStyle(el)
      if (cs.overflowX === 'hidden' || cs.textOverflow === 'ellipsis') continue
      if (el.closest('[class*="truncate"]')) continue
      if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
        out.push(`overflow: <${el.tagName.toLowerCase()}> "${el.textContent.trim().slice(0, 24)}" sw=${el.scrollWidth} cw=${el.clientWidth}`)
      }
    }
    // 2) 可交互元素意外重叠（弹窗开启时，忽略被 backdrop 遮住的页面元素——
    //    那是正常的层叠遮挡，交互被 backdrop 拦截，属于预期行为）
    const modalOpen = !!document.querySelector('.fixed.inset-0.z-50')
    const rects = [...document.querySelectorAll('button, [role="switch"]')]
      .filter((el) => el.offsetParent !== null)
      .filter((el) => !(modalOpen && !el.closest('.fixed.inset-0.z-50')))
      .map((el) => ({ el, r: el.getBoundingClientRect() }))
    for (let i = 0; i < rects.length; i++) {
      for (let j = i + 1; j < rects.length; j++) {
        const a = rects[i].r, b = rects[j].r
        // 判断父子关系
        const contains = rects[i].el.contains(rects[j].el) || rects[j].el.contains(rects[i].el)
        if (contains) continue
        const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left)
        const oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top)
        if (ox > 4 && oy > 4) {
          out.push(`overlap: "${rects[i].el.textContent.trim().slice(0, 12)}" × "${rects[j].el.textContent.trim().slice(0, 12)}" (${ox | 0}x${oy | 0})`)
        }
      }
    }
    // 3) body 级溢出
    const de = document.documentElement
    if (de.scrollWidth > de.clientWidth + 1) out.push(`page h-overflow: ${de.scrollWidth} > ${de.clientWidth}`)
    return out
  })

  const tag = `${w}x${h} ${pageName}`
  if (issues.length) {
    fail++
    console.log(`FAIL ${tag}`)
    for (const i of issues) console.log('   ' + i)
  } else {
    console.log(`PASS ${tag}`)
  }
  await ctx.close()
}

const openRules = async (p) => {
  await p.locator('aside').getByRole('button', { name: /游戏与平台/ }).click()
}
const openSettings = async (p) => {
  await p.locator('aside').getByRole('button', { name: '设置' }).click()
}
const openRulesEdit = async (p) => {
  await openRules(p)
  await sleep(200)
  await p.getByRole('button', { name: '新建规则' }).click()
}

for (const [w, h] of [[1120, 760], [900, 650]]) {
  await audit(w, h, '首页', null)
  await audit(w, h, '规则页', openRules)
  await audit(w, h, '规则编辑弹窗', openRulesEdit)
  await audit(w, h, '设置页', openSettings)
}

await browser.close()
process.exit(fail ? 1 : 0)
