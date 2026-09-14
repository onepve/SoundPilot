/**
 * 前后端契约门禁。
 *
 * 背景：Rust 枚举默认按变体名序列化（PascalCase），而前端 types.ts 约定小写字符串。
 * 一旦 Rust 侧漏掉 #[serde(rename_all = "lowercase")]，前端保存的配置会被 serde 拒绝，
 * 且只在真机保存时才暴露。这里直接读 Rust 源码做静态断言，把契约漂移挡在提交之前。
 */
import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'

const RUST_DIR = fileURLToPath(new URL('../src-tauri/src', import.meta.url))

function readRust(name: string): string {
  return readFileSync(`${RUST_DIR}/${name}`, 'utf8')
}

/** 取枚举定义前后的源码片段（含属性行）。 */
function enumBlock(src: string, enumName: string): string {
  const idx = src.indexOf(`pub enum ${enumName}`)
  expect(idx, `未找到枚举 ${enumName}`).toBeGreaterThan(-1)
  // 向上取 3 行属性、向下取到枚举结束
  const before = src.slice(0, idx).split('\n').slice(-4).join('\n')
  const after = src.slice(idx, src.indexOf('}', idx) + 1)
  return `${before}\n${after}`
}

describe('前后端枚举契约', () => {
  const config = readRust('config.rs')

  // 与 src/types.ts 的一一对应关系
  const contract: Array<[string, string[]]> = [
    ['MatchMode', ["'order' | 'volume'"]],
    ['RestoreMode', ["'normal' | 'previous' | 'none'"]],
    ['RuleKind', ["'game' | 'platform'"]],
  ]

  for (const [enumName] of contract) {
    it(`${enumName} 必须有 lowercase 序列化（否则前端字符串被拒）`, () => {
      const block = enumBlock(config, enumName)
      expect(
        block.includes('rename_all = "lowercase"'),
        `${enumName} 缺少 #[serde(rename_all = "lowercase")]，与前端 types.ts 契约不符`,
      ).toBe(true)
    })
  }

  it('Config 结构体字段为 camelCase', () => {
    expect(config.includes('rename_all = "camelCase"')).toBe(true)
  })

  it('前端 types.ts 的小写字面量与 Rust as_str() 一致', () => {
    const types = readFileSync(fileURLToPath(new URL('../src/types.ts', import.meta.url)), 'utf8')
    // as_str() 的小写返回值必须逐一出现在前端联合类型里
    const asStrValues = [...config.matchAll(/=>\s*"([a-z]+)"/g)].map((m) => m[1])
    expect(asStrValues.length).toBeGreaterThanOrEqual(6)
    for (const v of asStrValues) {
      expect(types, `前端 types.ts 缺少字面量 ${v}`).toContain(`'${v}'`)
    }
  })

  it('safe_default 默认启用监控（startPaused=false）', () => {
    const safeDefault = config.slice(config.indexOf('fn safe_default'))
    const body = safeDefault.slice(0, safeDefault.indexOf('}'))
    expect(body).toContain('start_paused: false')
    expect(body).not.toContain('start_paused: true')
  })

  it('AppState::new 采纳 startPaused 配置（不得硬编码 false）', () => {
    const state = readRust('state.rs')
    const idx = state.indexOf('pub fn new(config: Config)')
    const body = state.slice(idx, idx + 600)
    expect(
      body.includes('AtomicBool::new(start_paused)') || !body.includes('paused: AtomicBool::new(false)'),
      'AppState::new 忽略 startPaused 会导致开关失效',
    ).toBe(true)
  })
})

describe('Rust 命令注册与前端 bridge 对齐', () => {
  const lib = readRust('lib.rs')
  const bridge = readFileSync(fileURLToPath(new URL('../src/lib/bridge.ts', import.meta.url)), 'utf8')

  const commands = [
    'get_config',
    'save_config',
    'get_status',
    'list_processes',
    'pause_monitor',
    'resume_monitor',
    'refresh_volumes',
    'set_manual_volume',
    'pick_executable',
    'import_config',
    'export_config',
    'clear_logs',
    'quit_app',
  ]

  it('所有命令都在 Rust handler 注册', () => {
    const handler = lib.slice(lib.indexOf('generate_handler!['), lib.indexOf(']', lib.indexOf('generate_handler![')))
    for (const c of commands) {
      expect(handler, `${c} 未注册`).toContain(c)
    }
  })

  it('前端 bridge 覆盖全部命令', () => {
    for (const c of commands) {
      expect(bridge, `bridge 缺少 ${c}`).toContain(`'${c}'`)
    }
  })
})
