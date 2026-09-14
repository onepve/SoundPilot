import type { Config } from '@/types'

/** 配置数值字段的边界与默认值（与 Rust 端契约一致） */
export const LIMITS = {
  normalVolume: { min: 0, max: 100, def: 18 },
  checkInterval: { min: 1, max: 60, def: 3 },
  exitDebounce: { min: 1, max: 300, def: 15 },
} as const

/**
 * 校验整体配置（保存前最终闸门）。
 * 返回错误字符串数组；空数组 = 可保存。
 */
export function validateConfig(c: Config): string[] {
  const errs: string[] = []
  if (c.version !== 1) errs.push('配置版本必须为 1')
  if (!/^https?:\/\/.+/i.test(c.serverUrl.trim())) errs.push('服务器地址需以 http:// 或 https:// 开头')
  if (c.token && !/^[A-Za-z0-9_\-\.]{8,128}$/.test(c.token)) {
    errs.push('Token 格式可疑（应为 8–128 位字母数字符号）')
  }
  if (!Array.isArray(c.speakers) || c.speakers.length === 0) errs.push('至少配置一个音箱')
  const dids = new Set<string>()
  for (const s of c.speakers) {
    if (!s.did.trim()) errs.push('音箱 DID 不能为空')
    if (dids.has(s.did)) errs.push(`音箱 DID 重复：${s.did}`)
    dids.add(s.did)
  }
  if (!Number.isFinite(c.normalVolume) || c.normalVolume < 0 || c.normalVolume > 100) {
    errs.push('日常音量需在 0–100 之间')
  }
  if (c.matchMode !== 'order' && c.matchMode !== 'volume') errs.push('匹配模式无效')
  if (c.restoreMode !== 'normal' && c.restoreMode !== 'previous' && c.restoreMode !== 'none') {
    errs.push('回落模式无效')
  }
  const ci = LIMITS.checkInterval
  if (!Number.isFinite(c.checkInterval) || c.checkInterval < ci.min || c.checkInterval > ci.max) {
    errs.push(`检测间隔需在 ${ci.min}–${ci.max} 秒之间`)
  }
  const ed = LIMITS.exitDebounce
  if (!Number.isFinite(c.exitDebounce) || c.exitDebounce < ed.min || c.exitDebounce > ed.max) {
    errs.push(`退出缓冲需在 ${ed.min}–${ed.max} 秒之间`)
  }
  const ruleIds = new Set<string>()
  for (const r of c.rules) {
    if (!r.id) errs.push('规则缺少 id')
    if (ruleIds.has(r.id)) errs.push(`规则 id 重复：${r.name}`)
    ruleIds.add(r.id)
    if (!r.name.trim()) errs.push('存在未命名的规则')
    if (r.volume < 0 || r.volume > 100) errs.push(`规则「${r.name}」音量超出 0–100`)
    if (r.processes.length === 0) errs.push(`规则「${r.name}」未关联任何进程`)
  }
  return errs
}

/** 纯文本 token 脱敏：保留首尾各 2 位，长度不足显示 *** */
export function maskToken(token: string): string {
  const t = (token || '').trim()
  if (!t) return ''
  if (t.length <= 6) return '***'
  return `${t.slice(0, 2)}${'•'.repeat(Math.min(t.length - 4, 16))}${t.slice(-2)}`
}

/** 导出前的配置克隆：清空 token，规则/音箱结构保持 */
export function sanitizeForExport(c: Config): Config {
  return {
    ...structuredClone(c),
    token: '',
    speakers: c.speakers.map((s) => ({ ...s })),
    rules: c.rules.map((r) => ({
      ...r,
      processes: r.processes.map((p) => ({ ...p })),
    })),
  }
}

/** 比较导入配置与当前配置，生成给人看的差异摘要 */
export interface ImportDiff {
  addedRules: string[]
  changedRules: string[]
  removedRules: string[]
  speakerChanges: string[]
  settingsChanges: string[]
  tokenPresent: boolean
}

export function diffImport(current: Config, incoming: Config): ImportDiff {
  const curById = new Map(current.rules.map((r) => [r.id, r]))
  const incById = new Map(incoming.rules.map((r) => [r.id, r]))
  const addedRules: string[] = []
  const changedRules: string[] = []
  const removedRules: string[] = []
  for (const r of incoming.rules) {
    if (!curById.has(r.id)) addedRules.push(r.name)
    else {
      const old = curById.get(r.id)!
      if (
        old.name !== r.name ||
        old.kind !== r.kind ||
        old.enabled !== r.enabled ||
        old.volume !== r.volume ||
        JSON.stringify(old.processes) !== JSON.stringify(r.processes)
      ) {
        changedRules.push(r.name)
      }
    }
  }
  for (const r of current.rules) {
    if (!incById.has(r.id)) removedRules.push(r.name)
  }
  const speakerChanges: string[] = []
  const curSpk = new Map(current.speakers.map((s) => [s.did, s]))
  for (const s of incoming.speakers) {
    const old = curSpk.get(s.did)
    if (!old) speakerChanges.push(`新增音箱 ${s.name}(${s.did})`)
    else if (old.name !== s.name || old.enabled !== s.enabled) {
      speakerChanges.push(`音箱 ${s.name}(${s.did}) 变更`)
    }
  }
  for (const s of current.speakers) {
    if (!incoming.speakers.some((x) => x.did === s.did)) speakerChanges.push(`移除音箱 ${s.name}(${s.did})`)
  }
  const settingsChanges: string[] = []
  if (current.serverUrl !== incoming.serverUrl) settingsChanges.push('服务器地址')
  if (current.normalVolume !== incoming.normalVolume) settingsChanges.push('日常音量')
  if (current.matchMode !== incoming.matchMode) settingsChanges.push('匹配模式')
  if (current.restoreMode !== incoming.restoreMode) settingsChanges.push('回落模式')
  if (current.checkInterval !== incoming.checkInterval) settingsChanges.push('检测间隔')
  if (current.exitDebounce !== incoming.exitDebounce) settingsChanges.push('退出缓冲')
  if (current.startPaused !== incoming.startPaused) settingsChanges.push('启动即暂停')
  if (current.autoStart !== incoming.autoStart) settingsChanges.push('开机自启')
  return { addedRules, changedRules, removedRules, speakerChanges, settingsChanges, tokenPresent: !!incoming.token }
}
