<script setup lang="ts">
import { ref, watch } from 'vue'
import { Layers } from 'lucide-vue-next'
import { PLATFORM_TEMPLATES } from '@/lib/templates'
import { ruleFromTemplate } from '@/lib/rules'
import { store } from '@/store'
import AppModal from '@/components/ui/AppModal.vue'
import AppBanner from '@/components/ui/AppBanner.vue'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'update:open', v: boolean): void }>()

const added = ref<string | null>(null)

watch(
  () => props.open,
  (v) => {
    if (v) added.value = null
  },
)

function apply(t: (typeof PLATFORM_TEMPLATES)[number]) {
  const rule = ruleFromTemplate(t)
  store.mutateConfig((c) => {
    c.rules.push(rule)
  })
  added.value = t.name
  setTimeout(() => {
    emit('update:open', false)
  }, 650)
}

function close() {
  emit('update:open', false)
}
</script>

<template>
  <AppModal
    :open="open"
    title="平台模板"
    subtitle="快速创建常用游戏平台的规则"
    wide
    @close="close"
  >
    <div class="space-y-4">
      <AppBanner kind="info">
        模板仅预填这些平台<strong>常见的进程名</strong>，并不检测本机是否已安装对应平台；创建后可在规则里补充或修改进程。
      </AppBanner>

      <div v-if="added" class="rounded-xl bg-mint-50 px-4 py-3 text-sm font-medium text-mint-700">
        已添加「{{ added }}」规则（默认停用，可编辑后启用）
      </div>

      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <button
          v-for="t in PLATFORM_TEMPLATES"
          :key="t.id"
          class="group rounded-xl border border-slate-200 bg-white p-4 text-left transition hover:border-mint-400 hover:shadow-card-hover"
          @click="apply(t)"
        >
          <div class="mb-2 flex h-9 w-9 items-center justify-center rounded-lg bg-slate-100 transition group-hover:bg-mint-100">
            <Layers class="h-4.5 w-4.5 text-slate-500 transition group-hover:text-mint-600" />
          </div>
          <p class="text-sm font-semibold text-slate-800">{{ t.name }}</p>
          <p class="mt-0.5 text-xs text-slate-400">{{ t.description }}</p>
          <p class="mt-2 truncate font-mono text-[11px] text-slate-400">
            {{ t.processes.map((p) => p.name).join(', ') }}
          </p>
        </button>
      </div>
    </div>
  </AppModal>
</template>
