import { describe, it, expect } from 'vitest'
import { newId } from '@/lib/ids'
import { PLATFORM_TEMPLATES } from '@/lib/templates'
import { ruleFromTemplate, findProcessConflicts } from '@/lib/rules'
import type { Rule } from '@/types'

describe('newId', () => {
  it('生成带前缀的唯一 id', () => {
    const a = newId('rule')
    const b = newId('rule')
    expect(a).toMatch(/^rule_[a-z0-9]+$/)
    expect(a).not.toBe(b)
    expect(newId('x')).toMatch(/^x_/)
  })
})

describe('平台模板', () => {
  it('模板非空且结构合法', () => {
    expect(PLATFORM_TEMPLATES.length).toBeGreaterThanOrEqual(6)
    for (const t of PLATFORM_TEMPLATES) {
      expect(t.id).toBeTruthy()
      expect(t.name).toBeTruthy()
      expect(t.description).toBeTruthy()
      expect(t.processes.length).toBeGreaterThan(0)
      for (const p of t.processes) {
        expect(p.name.trim()).toBeTruthy()
      }
    }
  })

  it('模板 id 唯一', () => {
    const ids = new Set(PLATFORM_TEMPLATES.map((t) => t.id))
    expect(ids.size).toBe(PLATFORM_TEMPLATES.length)
  })
})

describe('进程冲突检测', () => {
  it('不同启用规则的相同进程报冲突', () => {
    const rules: Rule[] = [
      { id: 'a', name: 'A', kind: 'game', enabled: true, volume: 30, processes: [{ name: 'steam', path: '' }] },
      { id: 'b', name: 'B', kind: 'platform', enabled: true, volume: 20, processes: [{ name: 'Steam', path: '' }] },
    ]
    expect(findProcessConflicts(rules, '')).toEqual(['A ↔ B'])
  })

  it('禁用规则与自身不计入冲突；路径不同不冲突', () => {
    const rules: Rule[] = [
      { id: 'a', name: 'A', kind: 'game', enabled: true, volume: 30, processes: [{ name: 'x', path: 'C:/a/x.exe' }] },
      { id: 'b', name: 'B', kind: 'game', enabled: false, volume: 20, processes: [{ name: 'x', path: '' }] },
      { id: 'c', name: 'C', kind: 'game', enabled: true, volume: 20, processes: [{ name: 'x', path: 'D:/b/x.exe' }] },
    ]
    expect(findProcessConflicts(rules, '')).toEqual([])
  })
})
