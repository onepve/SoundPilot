<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  PauseCircle,
  PlayCircle,
  RefreshCw,
  Timer,
  Activity,
  Speaker,
  Clock3,
  ScrollText,
  Trash2,
} from 'lucide-vue-next'
import { store } from '@/store'
import AppButton from '@/components/ui/AppButton.vue'
import VolumeSlider from '@/components/ui/VolumeSlider.vue'
import ConfirmModal from '@/components/ui/ConfirmModal.vue'

const st = store.state

const pauseMinutes = ref(30)
const manualVolume = ref(18)
const selectedDid = ref<string>('') // '' = 全部音箱
const clearOpen = ref(false)
const clearBusy = ref(false)

/** 清空运行日志（仅内存缓冲，不影响监控与配置） */
async function doClearLogs() {
  clearOpen.value = false // ConfirmModal 不自关，由父组件负责
  clearBusy.value = true
  try {
    await store.clearLogs()
  } finally {
    clearBusy.value = false
  }
}

const pauseUntilText = computed(() => {
  const u = st.status?.pauseUntil ?? null
  if (!u) return null
  const d = new Date(u * 1000)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
})

const devices = computed(() => st.status?.devices ?? [])

const events = computed(() => st.events.slice(0, 200))

function fmtTime(iso: string): string {
  try {
    const d = new Date(iso)
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(
      d.getSeconds(),
    ).padStart(2, '0')}`
  } catch {
    return iso
  }
}

function levelCls(level: string): string {
  switch (level) {
    case 'error':
      return 'text-rose-600'
    case 'warn':
      return 'text-amber-600'
    case 'info':
      return 'text-sky-600'
    default:
      return 'text-mint-600'
  }
}

async function onPause(minutes: number | null) {
  await store.pause(minutes)
}
</script>

<template>
  <div class="mx-auto max-w-[980px] space-y-5 px-8 py-6">
    <!-- 状态总览卡 -->
    <section class="rounded-2xl bg-white p-6 shadow-card">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div class="flex items-center gap-4">
          <div
            class="flex h-14 w-14 items-center justify-center rounded-2xl"
            :class="st.offline ? 'bg-slate-100' : st.status?.paused ? 'bg-amber-50' : 'bg-mint-50'"
          >
            <component
              :is="st.offline ? Activity : st.status?.paused ? PauseCircle : Activity"
              class="h-7 w-7"
              :class="st.offline ? 'text-slate-400' : st.status?.paused ? 'text-amber-500' : 'text-mint-600'"
            />
          </div>
          <div>
            <p class="text-2xl font-bold text-slate-900">
              {{
                st.offline
                  ? '桌面功能不可用'
                  : st.status?.paused
                    ? '监控已暂停'
                    : '监控运行中'
              }}
            </p>
            <p class="mt-0.5 text-sm text-slate-500">
              <template v-if="st.offline">通过普通浏览器访问时无法连接桌面服务</template>
              <template v-else-if="st.status?.activeRuleName">
                当前命中：<span class="font-medium text-mint-700">{{ st.status.activeRuleName }}</span>
                · 目标音量 {{ st.status.targetVolume ?? '—' }}
              </template>
              <template v-else>暂无命中的规则，按设置的回落方式处理；启动不强制调音</template>
            </p>
          </div>
        </div>

        <div v-if="!st.offline" class="flex flex-wrap items-center gap-2">
          <template v-if="st.status?.paused">
            <AppButton variant="primary" @click="store.resume()">
              <PlayCircle class="h-4 w-4" /> 恢复监控
            </AppButton>
            <span
              v-if="pauseUntilText"
              class="inline-flex items-center gap-1 rounded-full bg-amber-50 px-3 py-1 text-xs font-medium text-amber-700"
            >
              <Clock3 class="h-3.5 w-3.5" /> 定时恢复于 {{ pauseUntilText }}
            </span>
          </template>
          <template v-else>
            <AppButton variant="secondary" @click="onPause(null)">
              <PauseCircle class="h-4 w-4" /> 暂停（手动恢复）
            </AppButton>
          </template>
          <AppButton variant="ghost" @click="store.refreshNow()">
            <RefreshCw class="h-4 w-4" /> 刷新音量
          </AppButton>
        </div>
      </div>

      <!-- 定时暂停 -->
      <div
        v-if="!st.offline && !st.status?.paused"
        class="mt-5 flex flex-wrap items-center gap-3 rounded-xl bg-slate-50 px-4 py-3"
      >
        <Timer class="h-4.5 w-4.5 text-slate-500" />
        <span class="text-sm text-slate-600">定时暂停</span>
        <div class="flex items-center gap-1">
          <button
            v-for="m in [15, 30, 60, 120]"
            :key="m"
            class="rounded-lg px-3 py-1 text-[13px] font-medium transition"
            :class="
              pauseMinutes === m
                ? 'bg-mint-600 text-white shadow-sm'
                : 'bg-white text-slate-600 hover:bg-slate-100 border border-slate-200'
            "
            @click="pauseMinutes = m"
          >
            {{ m >= 60 ? `${m / 60} 小时` : `${m} 分钟` }}
          </button>
        </div>
        <AppButton size="sm" variant="secondary" @click="onPause(pauseMinutes)">
          <Clock3 class="h-3.5 w-3.5" /> 到点自动恢复
        </AppButton>
      </div>
    </section>

    <div class="grid grid-cols-1 gap-5 lg:grid-cols-5">
      <!-- 手动音量 -->
      <section class="rounded-2xl bg-white p-6 shadow-card lg:col-span-2">
        <div class="mb-4 flex items-center gap-2">
          <Speaker class="h-5 w-5 text-mint-600" />
          <h3 class="font-semibold text-slate-900">手动音量</h3>
        </div>
        <p class="mb-4 text-[13px] leading-relaxed text-slate-500">
          手动设置会<strong class="text-slate-700">先暂停自动监控</strong>，避免被规则立即覆盖；需要恢复自动调音请点「恢复监控」。
        </p>

        <label class="mb-4 block">
          <span class="mb-1.5 block text-sm font-medium text-slate-700">目标音箱</span>
          <select
            v-model="selectedDid"
            :disabled="st.offline || devices.length === 0"
            class="w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm focus:border-mint-500 focus:outline-none disabled:bg-slate-50"
          >
            <option value="">全部启用的音箱</option>
            <option v-for="d in devices" :key="d.did" :value="d.did">
              {{ d.name }}（{{ d.did }}）
            </option>
          </select>
        </label>

        <VolumeSlider
          v-model="manualVolume"
          label="音量"
          suffix="%"
          :min="0"
          :max="100"
          :disabled="st.offline"
        />

        <AppButton block class="mt-5" :disabled="st.offline" @click="store.setManualVolume(manualVolume, selectedDid || null)">
          应用到音箱
        </AppButton>

        <!-- 设备读数 -->
        <div v-if="devices.length" class="mt-5 space-y-2 border-t border-slate-100 pt-4">
          <div v-for="d in devices" :key="d.did" class="flex items-center justify-between text-[13px]">
            <span class="truncate text-slate-600">{{ d.name }}</span>
            <span v-if="d.error" class="text-rose-500">{{ d.error }}</span>
            <span v-else-if="d.volume !== null" class="font-medium tabular-nums text-slate-800">
              {{ d.volume }}%
            </span>
            <span v-else class="text-slate-400">未读取</span>
          </div>
        </div>
      </section>

      <!-- 运行日志 -->
      <section class="flex min-h-[320px] flex-col rounded-2xl bg-white p-6 shadow-card lg:col-span-3">
        <div class="mb-4 flex items-center gap-2">
          <ScrollText class="h-5 w-5 text-mint-600" />
          <h3 class="font-semibold text-slate-900">运行日志</h3>
          <span class="rounded-md bg-slate-100 px-1.5 py-0.5 text-[11px] text-slate-500">
            最多保留 200 条
          </span>
          <AppButton
            v-if="!st.offline || st.demo"
            variant="secondary"
            class="ml-auto"
            :disabled="events.length === 0 || clearBusy"
            @click="clearOpen = true"
          >
            <Trash2 class="h-4 w-4" /> 清空日志
          </AppButton>
          <span v-else class="ml-auto text-xs text-slate-400">桌面运行后显示</span>
        </div>
        <div
          class="min-h-0 flex-1 space-y-1 overflow-y-auto rounded-xl bg-slate-50 p-4 font-mono text-[12px] leading-relaxed"
        >
          <p v-if="events.length === 0" class="py-8 text-center text-slate-500">
            暂无日志。启动监控、命中规则或暂停恢复时，事件会显示在这里。
          </p>
          <div v-for="(e, i) in events" :key="i" class="flex gap-2">
            <span class="shrink-0 text-slate-500">{{ fmtTime(e.time) }}</span>
            <span class="shrink-0 font-semibold uppercase" :class="levelCls(e.level)">{{ e.level }}</span>
            <span class="min-w-0 break-all text-slate-700">{{ e.message }}</span>
          </div>
        </div>
      </section>
    </div>

    <ConfirmModal
      v-model:open="clearOpen"
      title="清空运行日志"
      message="确定清空当前运行日志？日志仅保存在内存中（最多 200 条，不写磁盘），清空不影响监控运行与配置。"
      confirm-text="清空"
      @confirm="doClearLogs"
    />
  </div>
</template>
