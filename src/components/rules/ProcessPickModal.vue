<script setup lang="ts">
import { computed, watch, ref } from 'vue'
import { Search, RefreshCw, AppWindow, ListTree } from 'lucide-vue-next'
import type { ProcessInfo } from '@/types'
import { bridge, hasBridge } from '@/lib/bridge'
import AppModal from '@/components/ui/AppModal.vue'
import AppBanner from '@/components/ui/AppBanner.vue'
import ToggleSwitch from '@/components/ui/ToggleSwitch.vue'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'select', p: { name: string; path: string }): void
}>()

const query = ref('')
const loading = ref(false)
const error = ref('')
const processes = ref<ProcessInfo[]>([])
const allProcesses = ref(false) // false = 仅显示有窗口的

const visible = computed(() => {
  const q = query.value.trim().toLowerCase()
  return processes.value
    .filter((p) => allProcesses.value || p.hasWindow)
    .filter(
      (p) =>
        !q ||
        p.name.toLowerCase().includes(q) ||
        p.title.toLowerCase().includes(q) ||
        p.path.toLowerCase().includes(q),
    )
    .sort((a, b) => a.name.localeCompare(b.name))
})

async function load() {
  if (!hasBridge()) {
    error.value = '需要桌面环境才能枚举进程'
    return
  }
  loading.value = true
  error.value = ''
  try {
    processes.value = await bridge.listProcesses()
  } catch (err) {
    error.value = String((err as Error)?.message ?? err)
  } finally {
    loading.value = false
  }
}

watch(() => props.open, (open) => { if (open) void load() }, { immediate: true })

function onOpen(v: boolean) {
  if (v) load()
  emit('update:open', v)
}

function choose(p: ProcessInfo) {
  emit('select', { name: p.name, path: p.path })
  emit('update:open', false)
}
</script>

<template>
  <AppModal
    :open="open"
    title="从运行中的程序选择"
    subtitle="选择一个正在运行的进程，自动填入进程名与完整路径"
    wide
    @close="onOpen(false)"
  >
    <div class="space-y-3">
      <div class="flex items-center gap-2">
        <div class="relative min-w-0 flex-1">
          <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
          <input
            v-model="query"
            type="text"
            placeholder="搜索进程名、窗口标题或路径…"
            class="w-full rounded-lg border border-slate-200 py-2 pl-9 pr-3 text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
          />
        </div>
        <ToggleSwitch v-model="allProcesses" label="显示全部进程" small />
        <span class="whitespace-nowrap text-xs text-slate-500">
          {{ allProcesses ? '全部进程' : '仅有窗口' }}
        </span>
        <button
          class="rounded-lg border border-slate-200 p-2 text-slate-500 transition hover:bg-slate-50"
          title="重新加载"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>

      <div class="flex items-center justify-between text-xs text-slate-400">
        <span class="inline-flex items-center gap-1"><AppWindow class="h-3.5 w-3.5" />有窗口的程序优先显示</span>
        <span class="inline-flex items-center gap-1"><ListTree class="h-3.5 w-3.5" />共 {{ visible.length }} 项</span>
      </div>

      <div
        class="min-h-[260px] max-h-[46vh] space-y-1 overflow-y-auto rounded-xl border border-slate-100 bg-slate-50/40 p-2"
      >
        <AppBanner v-if="!hasBridge()" kind="warn">
          普通浏览器无法枚举系统进程，此功能仅在 SoundPilot 桌面程序中可用。
        </AppBanner>
        <p v-else-if="loading" class="py-10 text-center text-sm text-slate-400">正在读取进程列表…</p>
        <p v-else-if="error" class="py-10 text-center text-sm text-rose-500">{{ error }}</p>
        <p v-else-if="visible.length === 0" class="py-10 text-center text-sm text-slate-400">没有匹配的进程</p>

        <button
          v-for="p in visible"
          :key="p.pid"
          class="flex w-full items-center gap-3 rounded-lg border border-transparent bg-white px-3 py-2 text-left shadow-sm transition hover:border-mint-300 hover:bg-mint-50/40"
          @click="choose(p)"
        >
          <div class="min-w-0 flex-1">
            <p class="truncate font-mono text-[13px] font-medium text-slate-800">
              {{ p.name }}
              <span v-if="p.hasWindow" class="ml-1 rounded bg-sky-50 px-1 text-[10px] font-sans text-sky-500">窗口</span>
            </p>
            <p class="truncate text-[11px] text-slate-400">{{ p.title || p.path || '—' }}</p>
          </div>
          <span class="shrink-0 font-mono text-[11px] tabular-nums text-slate-300">PID {{ p.pid }}</span>
        </button>
      </div>
    </div>
  </AppModal>
</template>
