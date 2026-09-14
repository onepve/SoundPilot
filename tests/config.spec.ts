import { describe, it, expect } from 'vitest'
import { validateConfig, maskToken, sanitizeForExport, diffImport } from '@/lib/config'
import type { Config } from '@/types'

function mkConfig(p: Partial<Config> = {}): Config {
  return {
    version: 1,
    serverUrl: 'http://192.168.1.10:8080',
    token: 'abcd1234efgh5678',
    speakers: [{ did: 'did-1', name: '客厅', enabled: true }],
    normalVolume: 18,
    matchMode: 'order',
    restoreMode: 'normal',
    checkInterval: 3,
    exitDebounce: 15,
    startPaused: true,
    autoStart: false,
    rules: [
      {
        id: 'r1',
        name: 'LOL',
        kind: 'game',
        enabled: true,
        volume: 40,
        processes: [{ name: 'LeagueClient', path: '' }],
      },
    ],
    ...p,
  }
}

describe('配置校验', () => {
  it('合法配置通过', () => {
    expect(validateConfig(mkConfig())).toEqual([])
  })

  it('服务器地址格式', () => {
    expect(validateConfig(mkConfig({ serverUrl: '192.168.1.1' })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ serverUrl: 'ftp://x' })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ serverUrl: 'https://api.example.com' }))).toEqual([])
  })

  it('空 Token 合法（未配置时允许）', () => {
    expect(validateConfig(mkConfig({ token: '' }))).toEqual([])
  })

  it('音箱 DID 重复/为空报错', () => {
    const c = mkConfig({
      speakers: [
        { did: 'd1', name: 'a', enabled: true },
        { did: 'd1', name: 'b', enabled: true },
      ],
    })
    expect(validateConfig(c).some((e) => e.includes('重复'))).toBe(true)
    const c2 = mkConfig({ speakers: [{ did: ' ', name: 'a', enabled: true }] })
    expect(validateConfig(c2).some((e) => e.includes('不能为空'))).toBe(true)
  })

  it('数值边界：间隔/缓冲/音量', () => {
    expect(validateConfig(mkConfig({ checkInterval: 0 })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ checkInterval: 61 })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ exitDebounce: 301 })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ normalVolume: 101 })).length).toBeGreaterThan(0)
  })

  it('枚举字段非法报错', () => {
    expect(validateConfig(mkConfig({ matchMode: 'random' as any })).length).toBeGreaterThan(0)
    expect(validateConfig(mkConfig({ restoreMode: 'keep' as any })).length).toBeGreaterThan(0)
  })

  it('规则缺进程/重名 id 报错', () => {
    const c = mkConfig()
    c.rules = [
      { id: 'r1', name: 'A', kind: 'game', enabled: true, volume: 10, processes: [] },
      { id: 'r1', name: 'B', kind: 'game', enabled: true, volume: 10, processes: [{ name: 'x', path: '' }] },
    ]
    const errs = validateConfig(c)
    expect(errs.some((e) => e.includes('id 重复'))).toBe(true)
    expect(errs.some((e) => e.includes('未关联任何进程'))).toBe(true)
  })
})

describe('Token 脱敏与导出', () => {
  it('maskToken 各长度', () => {
    expect(maskToken('')).toBe('')
    expect(maskToken('abc')).toBe('***')
    expect(maskToken('abcdefg')).toBe('ab•••fg')
    const long = 'a'.repeat(40)
    expect(maskToken(long).startsWith('aa')).toBe(true)
    expect(maskToken(long).endsWith('aa')).toBe(true)
    expect(maskToken(long).length).toBeLessThan(40)
  })

  it('导出清空 token 且深拷贝不回写', () => {
    const c = mkConfig()
    const out = sanitizeForExport(c)
    expect(out.token).toBe('')
    expect(c.token).toBe('abcd1234efgh5678')
    out.rules[0].name = '改了'
    expect(c.rules[0].name).toBe('LOL')
  })
})

describe('导入差异预览', () => {
  it('识别新增/修改/删除规则与设置变化', () => {
    const cur = mkConfig()
    const inc = mkConfig({
      normalVolume: 30,
      rules: [
        { id: 'r1', name: 'LOL 改', kind: 'game', enabled: true, volume: 50, processes: [{ name: 'LeagueClient', path: '' }] },
        { id: 'r2', name: 'Steam', kind: 'platform', enabled: false, volume: 20, processes: [{ name: 'steam', path: '' }] },
      ],
    })
    const d = diffImport(cur, inc)
    expect(d.addedRules).toEqual(['Steam'])
    expect(d.changedRules).toEqual(['LOL 改'])
    expect(d.removedRules).toEqual([])
    expect(d.settingsChanges).toEqual(['日常音量'])
  })

  it('检测被移除的规则', () => {
    const cur = mkConfig()
    const inc = mkConfig({ rules: [] })
    expect(diffImport(cur, inc).removedRules).toEqual(['LOL'])
  })

  it('tokenPresent 标记', () => {
    expect(diffImport(mkConfig(), mkConfig({ token: '' })).tokenPresent).toBe(false)
    expect(diffImport(mkConfig(), mkConfig()).tokenPresent).toBe(true)
  })
})
