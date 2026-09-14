import type { Config, ProcessInfo, Status } from '@/types'

/**
 * Tauri invoke 封装。
 * - 桌面内：直连 @tauri-apps/api/core invoke。
 * - 普通浏览器：window.__TAURI_INTERNALS__ 不存在 => BridgeUnavailable，
 *   由 store 转入离线模式并明确告知"桌面功能不可用"，绝不伪造数据。
 */

export class BridgeUnavailable extends Error {
  constructor() {
    super('Tauri 桥不可用：当前运行在普通浏览器中，桌面功能不可用')
    this.name = 'BridgeUnavailable'
  }
}

export function hasBridge(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasBridge()) throw new BridgeUnavailable()
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(cmd, args)
}

export const bridge = {
  getConfig: () => call<Config>('get_config'),
  saveConfig: (config: Config) => call<void>('save_config', { config }),
  getStatus: () => call<Status>('get_status'),
  listProcesses: () => call<ProcessInfo[]>('list_processes'),
  pauseMonitor: (minutes: number | null) => call<void>('pause_monitor', { minutes }),
  resumeMonitor: () => call<void>('resume_monitor'),
  refreshVolumes: () => call<Status>('refresh_volumes'),
  setManualVolume: (volume: number, did: string | null) =>
    call<Status>('set_manual_volume', { volume, did }),
  pickExecutable: () => call<ProcessInfo | null>('pick_executable'),
  importConfig: () => call<Config | null>('import_config'),
  exportConfig: () => call<string | null>('export_config'),
  clearLogs: () => call<number>('clear_logs'),
  quitApp: () => call<void>('quit_app'),
}
