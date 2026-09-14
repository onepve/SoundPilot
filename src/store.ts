/**
 * 极简 reactive store（不引 pinia，减少依赖面）。
 * 保存/加载经 bridge；无桥时 offline=true，UI 显示"桌面功能不可用"横幅。
 *
 * 演示模式（demo）：仅在 URL 带 ?demo=1 且无 Tauri 桥时启用的开发测试模式。
 * 明确标记"演示数据"，不保存、不连接任何服务；生产构建也可用但仅显示本地内存态。
 */
import { reactive } from 'vue'
import type { Config, LogEvent, Status } from '@/types'
import { bridge, hasBridge } from '@/lib/bridge'

export type ToastKind = 'info' | 'success' | 'error'

export interface Toast {
  id: number
  kind: ToastKind
  message: string
}

export interface State {
  ready: boolean
  offline: boolean
  offlineReason: string
  demo: boolean
  config: Config | null
  status: Status | null
  saving: boolean
  dirty: boolean
  toasts: Toast[]
  events: LogEvent[]
}

function demoRequested(): boolean {
  try {
    return import.meta.env.DEV && new URLSearchParams(window.location.search).get('demo') === '1'
  } catch {
    return false
  }
}

function demoConfig(): Config {
  return {
    version: 1,
    serverUrl: 'http://localhost:8080',
    token: '',
    speakers: [
      { did: 'demo-did-living', name: '客厅（演示）', enabled: true },
      { did: 'demo-did-bedroom', name: '卧室（演示）', enabled: false },
    ],
    normalVolume: 18,
    matchMode: 'order',
    restoreMode: 'normal',
    checkInterval: 3,
    exitDebounce: 15,
    startPaused: false,
    autoStart: false,
    rules: [
      {
        id: 'demo-rule-1',
        name: '英雄联盟（演示）',
        kind: 'game',
        enabled: true,
        volume: 40,
        processes: [
          { name: 'LeagueClient', path: '' },
          { name: 'League of Legends', path: '' },
        ],
      },
      {
        id: 'demo-rule-2',
        name: 'Steam（演示）',
        kind: 'platform',
        enabled: false,
        volume: 25,
        processes: [{ name: 'steam', path: '' }],
      },
    ],
  }
}

function demoStatus(): Status {
  return {
    paused: false,
    pauseUntil: null,
    activeRuleId: null,
    activeRuleName: null,
    targetVolume: null,
    connection: '演示模式（未连接）',
    devices: [
      { did: 'demo-did-living', name: '客厅（演示）', volume: 18, error: null },
      { did: 'demo-did-bedroom', name: '卧室（演示）', volume: null, error: '演示设备不可达' },
    ],
    events: [
      { time: new Date().toISOString(), level: 'info', message: '演示模式启动：所有数据均为本地虚构，不连接任何服务' },
    ],
    configPath: '（演示模式，无配置文件）',
    version: '0.1.0-demo',
  }
}

const state = reactive<State>({
  ready: false,
  offline: !hasBridge(),
  offlineReason: '当前运行在普通浏览器中，未检测到 Tauri 桌面环境',
  demo: false,
  config: null,
  status: null,
  saving: false,
  dirty: false,
  toasts: [],
  events: [],
})

let toastSeq = 0

function toast(kind: ToastKind, message: string) {
  toastSeq += 1
  const id = toastSeq
  state.toasts.push({ id, kind, message })
  setTimeout(() => {
    const i = state.toasts.findIndex((t) => t.id === id)
    if (i !== -1) state.toasts.splice(i, 1)
  }, 4200)
}

async function refreshStatus() {
  if (state.offline) return
  try {
    state.status = await bridge.getStatus()
    if (state.status?.events) {
      const map = new Map(state.events.map((e) => [e.time + e.message, true]))
      for (const e of state.status.events) {
        if (!map.has(e.time + e.message)) state.events.unshift(e)
      }
      if (state.events.length > 500) state.events.length = 500
    }
  } catch (err) {
    // 状态轮询失败不弹 toast，只记录
    console.warn('refreshStatus failed', err)
  }
}

export const store = {
  state,

  get offline() {
    return state.offline
  },

  async init() {
    if (state.ready) return
    if (!hasBridge()) {
      if (demoRequested()) {
        // 演示模式：仅用于开发/测试 UI，不连接任何服务，不可保存
        state.demo = true
        state.config = demoConfig()
        state.status = demoStatus()
        state.events = [...demoStatus().events]
        state.ready = true
        return
      }
      state.offline = true
      state.ready = true
      return
    }
    try {
      state.config = await bridge.getConfig()
      await refreshStatus()
      state.ready = true
      setInterval(refreshStatus, 4000)
    } catch (err) {
      state.offline = true
      state.offlineReason = `桌面连接失败：${String((err as Error)?.message ?? err)}`
      state.ready = true
    }
  },

  markDirty() {
    state.dirty = true
  },

  mutateConfig(fn: (c: Config) => void) {
    if (!state.config) return
    fn(state.config)
    state.dirty = true
  },

  async save(): Promise<boolean> {
    if (!state.config || state.offline) {
      if (state.demo) toast('info', '演示模式：配置仅存在内存中，不会保存')
      return false
    }
    state.saving = true
    try {
      await bridge.saveConfig(state.config)
      state.dirty = false
      toast('success', '配置已保存')
      return true
    } catch (err) {
      toast('error', `保存失败：${String((err as Error)?.message ?? err)}`)
      return false
    } finally {
      state.saving = false
    }
  },

  async refreshNow() {
    if (state.demo) {
      toast('info', '演示模式：不会连接真实音箱')
      return
    }
    if (state.offline) return
    try {
      state.status = await bridge.refreshVolumes()
      toast('success', '音量已刷新')
    } catch (err) {
      toast('error', `刷新失败：${String((err as Error)?.message ?? err)}`)
    }
  },

  async pause(minutes: number | null) {
    if (state.demo) {
      state.status = state.status ? { ...state.status, paused: true } : state.status
      toast('info', '演示模式：仅切换界面状态')
      return
    }
    if (state.offline) return
    try {
      await bridge.pauseMonitor(minutes)
      await refreshStatus()
    } catch (err) {
      toast('error', `暂停失败：${String((err as Error)?.message ?? err)}`)
    }
  },

  async resume() {
    if (state.demo) {
      state.status = state.status ? { ...state.status, paused: false } : state.status
      toast('info', '演示模式：仅切换界面状态')
      return
    }
    if (state.offline) return
    try {
      await bridge.resumeMonitor()
      await refreshStatus()
    } catch (err) {
      toast('error', `恢复失败：${String((err as Error)?.message ?? err)}`)
    }
  },

  /** 清空运行日志（仅内存缓冲；不影响监控运行与配置） */
  async clearLogs() {
    if (state.demo) {
      const n = state.events.length
      state.events = []
      toast('info', `演示模式：已清空界面日志（${n} 条）`)
      return
    }
    if (state.offline) return
    try {
      const cleared = await bridge.clearLogs()
      await refreshStatus()
      toast('success', `已清空 ${cleared} 条日志`)
    } catch (err) {
      toast('error', `清空日志失败：${String((err as Error)?.message ?? err)}`)
    }
  },

  async setManualVolume(volume: number, did: string | null) {
    if (state.demo) {
      toast('info', `演示模式：不会真实写入音箱音量（${volume}%）`)
      return
    }
    if (state.offline) return
    try {
      state.status = await bridge.setManualVolume(volume, did)
      toast('success', `已将${did ? '所选音箱' : '全部音箱'}设为 ${volume}`)
    } catch (err) {
      toast('error', `设置失败：${String((err as Error)?.message ?? err)}`)
    }
  },

  notify: toast,
}
