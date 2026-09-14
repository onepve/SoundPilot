import type { Rule, RuleKind, RuleProcess } from '@/types'
import { newId } from '@/lib/ids'

export interface RuleEditDraft {
  id: string
  name: string
  kind: RuleKind
  enabled: boolean
  volume: number
  processes: RuleProcess[]
}

/** 新建规则草稿 */
export function emptyDraft(kind: RuleKind = 'game'): RuleEditDraft {
  return {
    id: '',
    name: '',
    kind,
    enabled: true,
    volume: 30,
    processes: [{ name: '', path: '' }],
  }
}

/** 从现有规则生成编辑草稿（深拷贝，避免直接改 store） */
export function toDraft(rule: Rule): RuleEditDraft {
  return {
    id: rule.id,
    name: rule.name,
    kind: rule.kind,
    enabled: rule.enabled,
    volume: rule.volume,
    processes: rule.processes.map((p) => ({ ...p })),
  }
}

/** 复制规则：新 id、名称加"副本"、置于原规则之后 */
export function duplicateRule(rules: Rule[], id: string): { rules: Rule[]; newId: string } | null {
  const idx = rules.findIndex((r) => r.id === id)
  if (idx === -1) return null
  const src = rules[idx]
  const copy: Rule = {
    ...structuredClone(toDraft(src)),
    id: newId('rule'),
    name: dedupeCopyName(rules, src.name),
    enabled: false,
  }
  const next = [...rules]
  next.splice(idx + 1, 0, copy)
  return { rules: next, newId: copy.id }
}

/** 名称去重：名字 (副本)、名字 (副本 2)、名字 (副本 3)… */
export function dedupeCopyName(rules: Rule[], base: string): string {
  const names = new Set(rules.map((r) => r.name))
  if (!names.has(`${base} (副本)`)) return `${base} (副本)`
  for (let i = 2; i < 1000; i++) {
    const cand = `${base} (副本 ${i})`
    if (!names.has(cand)) return cand
  }
  return `${base} (副本 ${Date.now()})`
}

export interface ValidationIssues {
  name: string | null
  volume: string | null
  processes: string[]
}

/** 校验编辑草稿；返回每条进程一个错误字符串（空串=通过） */
export function validateDraft(d: RuleEditDraft): ValidationIssues {
  const name = !d.name.trim() ? '请填写规则名称' : null
  const volume =
    !Number.isFinite(d.volume) || d.volume < 0 || d.volume > 100 ? '音量需在 0–100 之间' : null
  const processes = d.processes.map((p) => {
    if (!p.name.trim() && !p.path.trim()) return '请填写进程名或选择程序路径'
    return ''
  })
  return { name, volume, processes }
}

export function draftIsValid(d: RuleEditDraft): boolean {
  const v = validateDraft(d)
  return v.name === null && v.volume === null && v.processes.every((e) => e === '')
}

/** 草稿 → 正式规则（ trimming / 归一化）。调用前应已通过 draftIsValid */
export function draftToRule(d: RuleEditDraft): Rule {
  const processes = d.processes
    .map((p) => ({ name: p.name.trim(), path: p.path.trim() }))
    .filter((p) => p.name || p.path)
    .map((p) => (p.path ? { name: p.name || basenameOf(p.path), path: p.path } : p))
  return {
    id: d.id || newId('rule'),
    name: d.name.trim(),
    kind: d.kind,
    enabled: d.enabled,
    volume: Math.round(clamp(d.volume, 0, 100)),
    processes,
  }
}

export function basenameOf(path: string): string {
  const norm = path.replace(/\\/g, '/')
  const base = norm.slice(norm.lastIndexOf('/') + 1)
  return base.replace(/\.exe$/i, '') || norm
}

/** 上移 / 下移；返回新数组（不可变），越界返回原数组 */
export function moveRule(rules: Rule[], id: string, dir: -1 | 1): Rule[] {
  const idx = rules.findIndex((r) => r.id === id)
  const target = idx + dir
  if (idx === -1 || target < 0 || target >= rules.length) return rules
  const next = [...rules]
  const [item] = next.splice(idx, 1)
  next.splice(target, 0, item)
  return next
}

export type RuleFilterKind = 'all' | RuleKind

/** 搜索 + 分类过滤；搜索匹配规则名与进程名/路径，大小写不敏感 */
export function filterRules(rules: Rule[], query: string, kind: RuleFilterKind): Rule[] {
  const q = query.trim().toLowerCase()
  return rules.filter((r) => {
    if (kind !== 'all' && r.kind !== kind) return false
    if (!q) return true
    if (r.name.toLowerCase().includes(q)) return true
    return r.processes.some(
      (p) => p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q),
    )
  })
}

/** 模板应用：生成一条禁用的新规则 */
export function ruleFromTemplate(t: {
  id: string
  name: string
  processes: RuleProcess[]
  volume?: number
}): Rule {
  return {
    id: newId('rule'),
    name: t.name,
    kind: 'platform',
    enabled: false,
    volume: t.volume ?? 30,
    processes: t.processes.map((p) => ({ ...p })),
  }
}

export function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n))
}

/** 检查同名进程是否与其它规则冲突（提示用，不阻断） */
export function findProcessConflicts(rules: Rule[], selfId: string): string[] {
  const seen = new Map<string, string>() // lowercase name/path -> 规则名
  const conflicts = new Set<string>()
  for (const r of rules) {
    if (r.id === selfId || !r.enabled) continue
    for (const p of r.processes) {
      const key = (p.path || p.name).toLowerCase()
      const owner = seen.get(key)
      if (owner && owner !== r.name) conflicts.add(`${owner} ↔ ${r.name}`)
      else seen.set(key, r.name)
    }
  }
  return [...conflicts]
}
