<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Server,
  Speaker,
  SlidersHorizontal,
  Download,
  Upload,
  Plus,
  Trash2,
  Eye,
  EyeOff,
  ShieldCheck,
} from 'lucide-vue-next'
import { store } from '@/store'
import { bridge, hasBridge } from '@/lib/bridge'
import { validateConfig } from '@/lib/config'
import type { Config } from '@/types'
import AppButton from '@/components/ui/AppButton.vue'
import AppBanner from '@/components/ui/AppBanner.vue'
import VolumeSlider from '@/components/ui/VolumeSlider.vue'
import ToggleSwitch from '@/components/ui/ToggleSwitch.vue'
import ConfirmModal from '@/components/ui/ConfirmModal.vue'
import ImportPreviewModal from '@/components/settings/ImportPreviewModal.vue'

const st = store.state
const cfg = computed<Config | null>(() => st.config)

const showToken = ref(false)
const tokenInput = ref('')
const tokenEdited = ref(false)

// 打开设置页时同步一次 token 显示
watch(() => cfg.value?.token, (v) => { tokenInput.value = v ?? '' }, { immediate: true })

function onTokenInput(v: string) {
  tokenInput.value = v
  tokenEdited.value = true
  store.mutateConfig((c) => {
    c.token = v.trim()
  })
}



const errors = computed(() => (cfg.value ? validateConfig(cfg.value) : []))

/* ---- 音箱管理 ---- */
const newDid = ref('')
const newName = ref('')

function addSpeaker() {
  const did = newDid.value.trim()
  if (!did) return
  store.mutateConfig((c) => {
    if (c.speakers.some((s) => s.did === did)) return
    c.speakers.push({ did, name: newName.value.trim() || did, enabled: true })
  })
  newDid.value = ''
  newName.value = ''
}

function removeSpeaker(did: string) {
  store.mutateConfig((c) => {
    c.speakers = c.speakers.filter((s) => s.did !== did)
  })
}

/* ---- 导入/导出 ---- */
const importOpen = ref(false)
const importDraft = ref<Config | null>(null)
const importBusy = ref(false)

const resetOpen = ref(false)

async function doImport() {
  if (!hasBridge()) {
    store.notify('error', '导入需要桌面环境')
    return
  }
  importBusy.value = true
  try {
    const parsed = await bridge.importConfig()
    if (parsed) {
      importDraft.value = parsed
      importOpen.value = true
    } else {
      store.notify('info', '已取消导入')
    }
  } catch (err) {
    store.notify('error', `导入失败：${String((err as Error)?.message ?? err)}`)
  } finally {
    importBusy.value = false
  }
}

function applyImport(next: Config) {
  store.mutateConfig((c) => {
    Object.assign(c, { ...next, token: next.token || c.token })
  })
  tokenEdited.value = false
  importOpen.value = false
  store.notify('success', '导入完成，请检查后保存配置')
}

async function doExport() {
  if (!cfg.value) return
  if (!hasBridge()) {
    store.notify('error', '导出需要桌面环境')
    return
  }
  try {
    // 导出内容脱敏：Token 清空
    const path = await bridge.exportConfig()
    if (path) store.notify('success', `已导出到 ${path}（已去除 Token）`)
    else store.notify('info', '已取消导出')
  } catch (err) {
    store.notify('error', `导出失败：${String((err as Error)?.message ?? err)}`)
  }
}
</script>

<template>
  <div v-if="cfg" class="mx-auto max-w-[880px] space-y-5 px-8 py-6">
    <!-- 连接设置 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <div class="mb-5 flex items-center gap-2">
        <Server class="h-5 w-5 text-mint-600" />
        <h3 class="font-semibold text-slate-900">服务连接</h3>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <label class="block">
          <span class="mb-1.5 block text-sm font-medium text-slate-700">服务器地址</span>
          <input
            :value="cfg.serverUrl"
            type="text"
            placeholder="http://192.168.1.10:8080"
            class="w-full rounded-lg border border-slate-200 px-3 py-2 font-mono text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
            @input="store.mutateConfig((c) => (c.serverUrl = ($event.target as HTMLInputElement).value.trim()))"
          />
          <p class="mt-1 text-xs text-slate-400">xiaoi 服务的 Webhook 地址（HTTP 或 HTTPS）</p>
        </label>

        <label class="block">
          <span class="mb-1.5 flex items-center gap-1.5 text-sm font-medium text-slate-700">
            访问 Token
            <button
              type="button"
              class="inline-flex items-center gap-1 text-xs font-normal text-slate-400 hover:text-slate-600"
              @click="showToken = !showToken"
            >
              <component :is="showToken ? EyeOff : Eye" class="h-3.5 w-3.5" />
              {{ showToken ? '隐藏' : '显示' }}
            </button>
          </span>
          <input
            :value="tokenInput"
            :type="showToken ? 'text' : 'password'"
            autocomplete="off"
            placeholder="未配置（可选）"
            class="w-full rounded-lg border border-slate-200 px-3 py-2 font-mono text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
            @input="onTokenInput(($event.target as HTMLInputElement).value)"
          />
          <p class="mt-1 flex items-center gap-1 text-xs text-slate-400">
            <ShieldCheck class="h-3.5 w-3.5" /> 仅保存在本机配置文件，导出时自动去除
          </p>
        </label>
      </div>
    </section>

    <!-- 音箱 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <div class="mb-5 flex items-center justify-between">
        <div class="flex items-center gap-2">
          <Speaker class="h-5 w-5 text-mint-600" />
          <h3 class="font-semibold text-slate-900">音箱设备</h3>
          <span class="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-500">
            {{ cfg.speakers.filter((s) => s.enabled).length }}/{{ cfg.speakers.length }} 启用
          </span>
        </div>
      </div>

      <div class="space-y-2">
        <div
          v-for="s in cfg.speakers"
          :key="s.did"
          class="flex items-center gap-3 rounded-xl border border-slate-100 bg-slate-50/50 px-4 py-3"
        >
          <ToggleSwitch
            :model-value="s.enabled"
            :label="`启用 ${s.name}`"
            @update:model-value="store.mutateConfig((c) => { const t = c.speakers.find((x) => x.did === s.did); if (t) t.enabled = !t.enabled })"
          />
          <div class="min-w-0 flex-1">
            <input
              :value="s.name"
              type="text"
              class="w-full rounded-md border border-transparent bg-transparent px-1.5 py-0.5 text-sm font-medium text-slate-800 transition hover:border-slate-200 focus:border-mint-500 focus:bg-white focus:outline-none"
              @change="store.mutateConfig((c) => { const t = c.speakers.find((x) => x.did === s.did); if (t) t.name = ($event.target as HTMLInputElement).value.trim() })"
            />
            <p class="px-1.5 font-mono text-xs text-slate-400">{{ s.did }}</p>
          </div>
          <button
            class="rounded-lg p-2 text-slate-300 transition hover:bg-rose-50 hover:text-rose-500"
            :aria-label="`删除音箱 ${s.name}`"
            @click="removeSpeaker(s.did)"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>

      <div class="mt-3 flex flex-wrap items-end gap-2">
        <label class="block flex-1 min-w-[180px]">
          <span class="mb-1 block text-xs font-medium text-slate-500">新音箱 DID</span>
          <input
            v-model="newDid"
            type="text"
            placeholder="例如 34939a12..."
            class="w-full rounded-lg border border-slate-200 px-3 py-1.5 font-mono text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
          />
        </label>
        <label class="block flex-1 min-w-[120px]">
          <span class="mb-1 block text-xs font-medium text-slate-500">名称</span>
          <input
            v-model="newName"
            type="text"
            placeholder="卧室"
            class="w-full rounded-lg border border-slate-200 px-3 py-1.5 text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
          />
        </label>
        <AppButton variant="secondary" size="sm" class="h-[34px]" :disabled="!newDid.trim()" @click="addSpeaker">
          <Plus class="h-4 w-4" /> 添加
        </AppButton>
      </div>
    </section>

    <!-- 调音行为 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <div class="mb-5 flex items-center gap-2">
        <SlidersHorizontal class="h-5 w-5 text-mint-600" />
        <h3 class="font-semibold text-slate-900">调音行为</h3>
      </div>

      <div class="grid grid-cols-1 gap-x-8 gap-y-5 md:grid-cols-2">
        <VolumeSlider
          :model-value="cfg.normalVolume"
          label="日常音量（无规则时回落）"
          :min="0"
          :max="100"
          @update:model-value="store.mutateConfig((c) => (c.normalVolume = $event))"
        />

        <div>
          <span class="mb-1.5 block text-sm font-medium text-slate-700">无规则时回落</span>
          <div class="flex rounded-lg border border-slate-200 p-1">
            <button
              v-for="m in [
                { id: 'normal', label: '回到日常音量' },
                { id: 'previous', label: '恢复原音量' },
                { id: 'none', label: '保持不动' },
              ] as const"
              :key="m.id"
              class="flex-1 rounded-md px-2 py-1.5 text-xs font-medium transition"
              :class="cfg.restoreMode === m.id ? 'bg-mint-600 text-white shadow-sm' : 'text-slate-600 hover:bg-slate-50'"
              @click="store.mutateConfig((c) => (c.restoreMode = m.id))"
            >
              {{ m.label }}
            </button>
          </div>
        </div>

        <div>
          <span class="mb-1.5 block text-sm font-medium text-slate-700">同组多规则命中</span>
          <div class="flex rounded-lg border border-slate-200 p-1">
            <button
              v-for="m in [
                { id: 'order', label: '按列表顺序' },
                { id: 'volume', label: '取最高音量' },
              ] as const"
              :key="m.id"
              class="flex-1 rounded-md px-2 py-1.5 text-xs font-medium transition"
              :class="cfg.matchMode === m.id ? 'bg-mint-600 text-white shadow-sm' : 'text-slate-600 hover:bg-slate-50'"
              @click="store.mutateConfig((c) => (c.matchMode = m.id))"
            >
              {{ m.label }}
            </button>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <label class="block">
            <span class="mb-1.5 block text-sm font-medium text-slate-700">检测间隔（秒）</span>
            <input
              :value="cfg.checkInterval"
              type="number"
              min="1"
              max="60"
              class="w-full rounded-lg border border-slate-200 px-3 py-2 text-sm tabular-nums focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
              @change="store.mutateConfig((c) => (c.checkInterval = Math.round(Number(($event.target as HTMLInputElement).value) || 3)))"
            />
          </label>
          <label class="block">
            <span class="mb-1.5 block text-sm font-medium text-slate-700">退出缓冲（秒）</span>
            <input
              :value="cfg.exitDebounce"
              type="number"
              min="1"
              max="300"
              class="w-full rounded-lg border border-slate-200 px-3 py-2 text-sm tabular-nums focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
              @change="store.mutateConfig((c) => (c.exitDebounce = Math.round(Number(($event.target as HTMLInputElement).value) || 15)))"
            />
          </label>
        </div>

        <div class="flex items-center justify-between rounded-xl border border-slate-100 bg-slate-50/60 px-4 py-3">
          <div>
            <p class="text-sm font-medium text-slate-700">启动时暂停监控</p>
            <p class="text-xs text-slate-400">启动后不自动调音，手动恢复</p>
          </div>
          <ToggleSwitch
            :model-value="cfg.startPaused"
            label="启动时暂停"
            @update:model-value="store.mutateConfig((c) => (c.startPaused = !c.startPaused))"
          />
        </div>

        <div class="flex items-center justify-between rounded-xl border border-slate-100 bg-slate-50/60 px-4 py-3">
          <div>
            <p class="text-sm font-medium text-slate-700">开机自动启动</p>
            <p class="text-xs text-slate-400">
              登录 Windows 后自动启动并直接最小化到托盘，静默监控、不弹窗打扰；从托盘菜单可随时打开主窗口
            </p>
          </div>
          <ToggleSwitch
            :model-value="cfg.autoStart"
            label="开机自启"
            @update:model-value="store.mutateConfig((c) => (c.autoStart = !c.autoStart))"
          />
        </div>
      </div>
    </section>

    <!-- 校验错误 -->
    <AppBanner v-if="errors.length" kind="error" title="配置存在问题，保存前请修正">
      <ul class="list-inside list-disc">
        <li v-for="e in errors" :key="e">{{ e }}</li>
      </ul>
    </AppBanner>

    <!-- 导入导出 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <div class="mb-4 flex items-center gap-2">
        <Download class="h-5 w-5 text-mint-600" />
        <h3 class="font-semibold text-slate-900">配置导入 / 导出</h3>
      </div>
      <div class="flex flex-wrap gap-3">
        <AppButton variant="secondary" :loading="importBusy" @click="doImport">
          <Upload class="h-4 w-4" /> 导入配置（INI / JSON）
        </AppButton>
        <AppButton variant="secondary" @click="doExport">
          <Download class="h-4 w-4" /> 导出配置（自动去除 Token）
        </AppButton>
      </div>
      <p class="mt-3 text-xs leading-relaxed text-slate-400">
        导入会先展示与当前配置的差异预览，确认后才写入；导出文件不含 Token，但仍包含服务器地址和程序路径，分享前请检查。
      </p>
    </section>

    <!-- 关于 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <h3 class="mb-2 font-semibold text-slate-900">关于</h3>
      <div class="grid grid-cols-2 gap-2 text-sm text-slate-500">
        <p>版本：<span class="font-medium text-slate-700">{{ st.status?.version ?? '—' }}</span></p>
        <p>配置文件：<span class="font-mono text-xs text-slate-600">{{ st.status?.configPath ?? '桌面环境可见' }}</span></p>
      </div>
    </section>

    <!-- 弹窗 -->
    <ImportPreviewModal
      v-model:open="importOpen"
      :incoming="importDraft"
      :current="cfg"
      @apply="applyImport"
    />

    <ConfirmModal
      v-model:open="resetOpen"
      title="恢复默认设置"
      message="确定将行为设置恢复为默认值吗？规则与音箱不受影响。"
      confirm-text="恢复默认"
      @confirm="resetOpen = false"
    />
  </div>

  <div v-else class="flex h-full items-center justify-center">
    <p class="text-slate-400">{{ st.offline ? '桌面功能不可用，请在桌面程序中打开设置。' : '配置加载中…' }}</p>
  </div>
</template>
