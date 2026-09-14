// SoundPilot 前端类型 — 与 src-tauri 命令契约一一对应（camelCase JSON）

export type MatchMode = 'order' | 'volume'
export type RestoreMode = 'normal' | 'previous' | 'none'
export type RuleKind = 'game' | 'platform'

export interface Speaker {
  did: string
  name: string
  enabled: boolean
}

export interface RuleProcess {
  name: string
  path: string
}

export interface Rule {
  id: string
  name: string
  kind: RuleKind
  enabled: boolean
  volume: number
  processes: RuleProcess[]
}

export interface Config {
  version: 1
  serverUrl: string
  token: string
  speakers: Speaker[]
  normalVolume: number
  matchMode: MatchMode
  restoreMode: RestoreMode
  checkInterval: number
  exitDebounce: number
  startPaused: boolean
  autoStart: boolean
  rules: Rule[]
}

export interface DeviceVolume {
  did: string
  name: string
  volume: number | null
  error: string | null
}

export interface LogEvent {
  time: string
  level: string
  message: string
}

export interface Status {
  paused: boolean
  pauseUntil: number | null
  activeRuleId: string | null
  activeRuleName: string | null
  targetVolume: number | null
  connection: string
  devices: DeviceVolume[]
  events: LogEvent[]
  configPath: string
  version: string
}

export interface ProcessInfo {
  pid: number
  name: string
  path: string
  title: string
  hasWindow: boolean
  icon?: string
}

/** 平台快速模板 —— 仅预填常见进程名，不代表检测到本机已安装 */
export interface PlatformTemplate {
  id: string
  name: string
  description: string
  processes: RuleProcess[]
}
