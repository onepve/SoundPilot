import { describe, it, expect } from 'vitest'
import {
  emptyDraft,
  toDraft,
  draftToRule,
  draftIsValid,
  validateDraft,
  duplicateRule,
  moveRule,
  filterRules,
  dedupeCopyName,
  basenameOf,
  ruleFromTemplate,
} from '@/lib/rules'
import type { Rule } from '@/types'

function mkRule(p: Partial<Rule> = {}): Rule {
  return {
    id: p.id ?? 'r1',
    name: p.name ?? '规则一',
    kind: p.kind ?? 'game',
    enabled: p.enabled ?? true,
    volume: p.volume ?? 30,
    processes: p.processes ?? [{ name: 'game', path: '' }],
  }
}

describe('规则草稿校验', () => {
  it('空名称报错', () => {
    const d = emptyDraft()
    d.name = '   '
    expect(validateDraft(d).name).toBe('请填写规则名称')
    expect(draftIsValid(d)).toBe(false)
  })

  it('音量越界报错', () => {
    const d = emptyDraft()
    d.name = 'x'
    d.volume = 101
    expect(validateDraft(d).volume).toBeTruthy()
    d.volume = -1
    expect(validateDraft(d).volume).toBeTruthy()
  })

  it('每个进程独立校验：空进程报错、填了名称即通过', () => {
    const d = emptyDraft()
    d.name = 'x'
    d.processes = [
      { name: '', path: '' },
      { name: 'a.exe', path: '' },
      { name: '', path: 'C:/x/b.exe' },
    ]
    const v = validateDraft(d)
    expect(v.processes).toEqual(['请填写进程名或选择程序路径', '', ''])
    expect(draftIsValid(d)).toBe(false)
    d.processes[0].name = 'zz'
    expect(draftIsValid(d)).toBe(true)
  })

  it('draftToRule：trim、剔除空行、按路径推导名称、钳制音量', () => {
    const d = emptyDraft()
    d.name = '  王者  '
    d.volume = 250
    d.processes = [
      { name: ' a ', path: ' ' },
      { name: '', path: 'C:\\Games\\Hero\\Hero.exe' },
      { name: '', path: '   ' },
    ]
    const r = draftToRule(d)
    expect(r.name).toBe('王者')
    expect(r.volume).toBe(100)
    expect(r.processes).toEqual([
      { name: 'a', path: '' },
      { name: 'Hero', path: 'C:\\Games\\Hero\\Hero.exe' },
    ])
    expect(r.id).toBeTruthy()
  })

  it('新建规则自动生成 id；编辑保留原 id', () => {
    const a = draftToRule(emptyDraft())
    expect(a.id).toMatch(/^rule_/)
    const edit = toDraft(mkRule({ id: 'keep_me' }))
    expect(draftToRule(edit).id).toBe('keep_me')
  })
})

describe('规则排序', () => {
  const rules = [mkRule({ id: 'a', name: 'A' }), mkRule({ id: 'b', name: 'B' }), mkRule({ id: 'c', name: 'C' })]

  it('上移/下移交换相邻位置', () => {
    expect(moveRule(rules, 'b', -1).map((r) => r.id)).toEqual(['b', 'a', 'c'])
    expect(moveRule(rules, 'b', 1).map((r) => r.id)).toEqual(['a', 'c', 'b'])
  })

  it('边界不移动且返回同一引用', () => {
    expect(moveRule(rules, 'a', -1)).toBe(rules)
    expect(moveRule(rules, 'c', 1)).toBe(rules)
  })

  it('未知 id 原样返回', () => {
    expect(moveRule(rules, 'zzz', 1)).toBe(rules)
  })

  it('不修改原数组', () => {
    moveRule(rules, 'a', 1)
    expect(rules.map((r) => r.id)).toEqual(['a', 'b', 'c'])
  })
})

describe('规则复制', () => {
  it('复制插入到原规则之后，默认禁用，名称加副本', () => {
    const rules = [mkRule({ id: 'a', name: 'LOL' }), mkRule({ id: 'b' })]
    const out = duplicateRule(rules, 'a')!
    expect(out.rules.map((r) => r.id)).toEqual(['a', out.newId, 'b'])
    const copy = out.rules[1]
    expect(copy.name).toBe('LOL (副本)')
    expect(copy.enabled).toBe(false)
    expect(copy.id).not.toBe('a')
  })

  it('重名时自动编号', () => {
    const rules = [mkRule({ id: 'a', name: 'X' }), mkRule({ id: 'b', name: 'X (副本)' })]
    expect(dedupeCopyName(rules, 'X')).toBe('X (副本 2)')
    const out = duplicateRule(rules, 'a')!
    expect(out.rules[1].name).toBe('X (副本 2)')
  })

  it('未知 id 返回 null', () => {
    expect(duplicateRule([mkRule()], 'nope')).toBeNull()
  })
})

describe('规则搜索与分类', () => {
  const rules = [
    mkRule({ id: 'a', name: '英雄联盟', kind: 'game', processes: [{ name: 'LeagueClient', path: '' }] }),
    mkRule({ id: 'b', name: 'Steam', kind: 'platform', processes: [{ name: 'steam', path: 'C:/steam/steam.exe' }] }),
    mkRule({ id: 'c', name: '原神', kind: 'game', processes: [{ name: 'YuanShen', path: '' }] }),
  ]

  it('按分类过滤', () => {
    expect(filterRules(rules, '', 'game').map((r) => r.id)).toEqual(['a', 'c'])
    expect(filterRules(rules, '', 'platform').map((r) => r.id)).toEqual(['b'])
    expect(filterRules(rules, '', 'all')).toHaveLength(3)
  })

  it('按名称搜索（大小写不敏感）', () => {
    expect(filterRules(rules, 'steam', 'all').map((r) => r.id)).toEqual(['b'])
    expect(filterRules(rules, 'STEAM', 'all')).toHaveLength(1)
  })

  it('按进程名/路径搜索', () => {
    expect(filterRules(rules, 'league', 'all').map((r) => r.id)).toEqual(['a'])
    expect(filterRules(rules, 'c:/steam', 'all').map((r) => r.id)).toEqual(['b'])
  })

  it('组合分类+搜索', () => {
    expect(filterRules(rules, 'steam', 'game')).toHaveLength(0)
    expect(filterRules(rules, 'steam', 'platform')).toHaveLength(1)
  })
})

describe('工具函数', () => {
  it('basenameOf 处理反斜杠与 exe 后缀', () => {
    expect(basenameOf('C:\\Program Files\\Steam\\steam.exe')).toBe('steam')
    expect(basenameOf('D:/Games/Hero.EXE')).toBe('Hero')
    expect(basenameOf('plainname')).toBe('plainname')
  })

  it('模板生成禁用平台规则', () => {
    const r = ruleFromTemplate({ id: 'steam', name: 'Steam 平台', processes: [{ name: 'steam', path: '' }] })
    expect(r.kind).toBe('platform')
    expect(r.enabled).toBe(false)
    expect(r.processes).toEqual([{ name: 'steam', path: '' }])
  })
})
