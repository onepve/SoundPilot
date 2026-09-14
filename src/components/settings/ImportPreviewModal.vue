<script setup lang="ts">
import { computed } from 'vue'
import { FileDiff, PlusCircle, PencilLine, MinusCircle, Settings2, KeyRound } from 'lucide-vue-next'
import type { Config } from '@/types'
import { diffImport, validateConfig } from '@/lib/config'
import AppModal from '@/components/ui/AppModal.vue'
import AppButton from '@/components/ui/AppButton.vue'
import AppBanner from '@/components/ui/AppBanner.vue'

const props = defineProps<{
  open: boolean
  incoming: Config | null
  current: Config
}>()

const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'apply', next: Config): void
}>()

const diff = computed(() => (props.incoming ? diffImport(props.current, props.incoming) : null))
const problems = computed(() => (props.incoming ? validateConfig(props.incoming) : []))

const totalChanges = computed(() => {
  const d = diff.value
  if (!d) return 0
  return (
    d.addedRules.length +
    d.changedRules.length +
    d.removedRules.length +
    d.speakerChanges.length +
    d.settingsChanges.length
  )
})

function row(icon: typeof PlusCircle, tone: string, text: string, key: string) {
  return { icon, tone, text, key }
}
</script>

<template>
  <AppModal
    :open="open && !!incoming"
    title="导入预览"
    subtitle="请核对以下差异，确认后才会覆盖当前配置"
    wide
    @close="emit('update:open', false)"
  >
    <div v-if="diff" class="space-y-5">
      <AppBanner :kind="totalChanges === 0 ? 'info' : 'warn'">
        {{
          totalChanges === 0
            ? '导入内容与当前配置基本一致。'
            : `共 ${totalChanges} 处变化，确认后将覆盖当前配置（未保存的修改会丢失）。`
        }}
      </AppBanner>

      <div class="rounded-xl border border-slate-100">
        <div class="flex items-center gap-2 border-b border-slate-100 px-4 py-2.5 text-sm font-semibold text-slate-700">
          <FileDiff class="h-4 w-4 text-slate-400" /> 规则
        </div>
        <div class="divide-y divide-slate-50">
          <div v-if="diff.addedRules.length + diff.changedRules.length + diff.removedRules.length === 0" class="px-4 py-3 text-sm text-slate-400">
            无变化
          </div>
          <div
            v-for="r in diff.addedRules.map((n) => row(PlusCircle, 'text-mint-600', `新增规则：${n}`, 'a' + n))"
            :key="r.key"
            class="flex items-center gap-2.5 px-4 py-2.5 text-sm"
          >
            <component :is="r.icon" class="h-4 w-4 shrink-0" :class="r.tone" />
            <span class="text-slate-700">{{ r.text }}</span>
          </div>
          <div
            v-for="r in diff.changedRules.map((n) => row(PencilLine, 'text-amber-500', `修改规则：${n}`, 'c' + n))"
            :key="r.key"
            class="flex items-center gap-2.5 px-4 py-2.5 text-sm"
          >
            <component :is="r.icon" class="h-4 w-4 shrink-0" :class="r.tone" />
            <span class="text-slate-700">{{ r.text }}</span>
          </div>
          <div
            v-for="r in diff.removedRules.map((n) => row(MinusCircle, 'text-rose-500', `移除规则：${n}`, 'r' + n))"
            :key="r.key"
            class="flex items-center gap-2.5 px-4 py-2.5 text-sm"
          >
            <component :is="r.icon" class="h-4 w-4 shrink-0" :class="r.tone" />
            <span class="text-slate-700">{{ r.text }}</span>
          </div>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div class="rounded-xl border border-slate-100">
          <div class="flex items-center gap-2 border-b border-slate-100 px-4 py-2.5 text-sm font-semibold text-slate-700">
            <Settings2 class="h-4 w-4 text-slate-400" /> 音箱与设置
          </div>
          <div class="divide-y divide-slate-50">
            <div
              v-if="diff.speakerChanges.length + diff.settingsChanges.length === 0"
              class="px-4 py-3 text-sm text-slate-400"
            >
              无变化
            </div>
            <p v-for="s in diff.speakerChanges" :key="s" class="px-4 py-2 text-[13px] text-slate-600">
              {{ s }}
            </p>
            <p v-for="s in diff.settingsChanges" :key="s" class="px-4 py-2 text-[13px] text-slate-600">
              {{ s }}变化
            </p>
          </div>
        </div>

        <div class="space-y-3">
          <AppBanner v-if="diff.tokenPresent" kind="warn">
            <span class="inline-flex items-center gap-1.5">
              <KeyRound class="h-3.5 w-3.5" /> 导入内容包含 Token，将写入本机配置（仅本地保存）
            </span>
          </AppBanner>
          <AppBanner v-else kind="info">导入内容不含 Token，当前 Token 保持不变</AppBanner>

          <AppBanner v-if="problems.length" kind="error" title="导入内容存在校验问题">
            <ul class="list-inside list-disc">
              <li v-for="p in problems" :key="p">{{ p }}</li>
            </ul>
          </AppBanner>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2.5">
        <AppButton variant="secondary" @click="emit('update:open', false)">取消</AppButton>
        <AppButton :disabled="problems.length > 0 || !incoming" @click="incoming && emit('apply', incoming)">
          确认导入
        </AppButton>
      </div>
    </template>
  </AppModal>
</template>
