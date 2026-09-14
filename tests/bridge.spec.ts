import { describe, it, expect, vi, beforeEach } from 'vitest'

/**
 * bridge 单元测试：验证 invoke 参数形状与无桥时的 BridgeUnavailable。
 * 不触碰真实网络/Tauri —— invoke 模块被 mock。
 */

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}))

import { bridge, hasBridge, BridgeUnavailable } from '@/lib/bridge'

describe('bridge', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    invokeMock.mockResolvedValue(undefined)
  })

  it('无桥时 hasBridge()=false 且所有调用抛 BridgeUnavailable', async () => {
    expect(hasBridge()).toBe(false) // node 环境无 window
    await expect(bridge.getConfig()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.getStatus()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.listProcesses()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.pauseMonitor(30)).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.resumeMonitor()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.refreshVolumes()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.setManualVolume(20, null)).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.pickExecutable()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.importConfig()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.exportConfig()).rejects.toBeInstanceOf(BridgeUnavailable)
    await expect(bridge.quitApp()).rejects.toBeInstanceOf(BridgeUnavailable)
  })

  it('有桥时命令名与参数符合契约（snake_case 命令 + camelCase 参数）', async () => {
    // 模拟 Tauri 环境
    ;(globalThis as any).window = { __TAURI_INTERNALS__: {} }
    try {
      expect(hasBridge()).toBe(true)

      await bridge.getConfig()
      expect(invokeMock).toHaveBeenLastCalledWith('get_config', undefined)

      await bridge.getStatus()
      expect(invokeMock).toHaveBeenLastCalledWith('get_status', undefined)

      await bridge.pauseMonitor(45)
      expect(invokeMock).toHaveBeenLastCalledWith('pause_monitor', { minutes: 45 })

      await bridge.pauseMonitor(null)
      expect(invokeMock).toHaveBeenLastCalledWith('pause_monitor', { minutes: null })

      await bridge.resumeMonitor()
      expect(invokeMock).toHaveBeenLastCalledWith('resume_monitor', undefined)

      await bridge.setManualVolume(20, 'did-1')
      expect(invokeMock).toHaveBeenLastCalledWith('set_manual_volume', { volume: 20, did: 'did-1' })

      await bridge.setManualVolume(5, null)
      expect(invokeMock).toHaveBeenLastCalledWith('set_manual_volume', { volume: 5, did: null })

      await bridge.pickExecutable()
      expect(invokeMock).toHaveBeenLastCalledWith('pick_executable', undefined)

      await bridge.importConfig()
      expect(invokeMock).toHaveBeenLastCalledWith('import_config', undefined)

      await bridge.exportConfig()
      expect(invokeMock).toHaveBeenLastCalledWith('export_config', undefined)

      await bridge.quitApp()
      expect(invokeMock).toHaveBeenLastCalledWith('quit_app', undefined)

      await bridge.listProcesses()
      expect(invokeMock).toHaveBeenLastCalledWith('list_processes', undefined)

      await bridge.refreshVolumes()
      expect(invokeMock).toHaveBeenLastCalledWith('refresh_volumes', undefined)

      const cfg = { version: 1 } as any
      await bridge.saveConfig(cfg)
      expect(invokeMock).toHaveBeenLastCalledWith('save_config', { config: cfg })
    } finally {
      delete (globalThis as any).window
    }
  })
})
