<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, CheckCircle2, Info, XCircle } from 'lucide-vue-next'

const props = defineProps<{
  kind?: 'info' | 'success' | 'error' | 'warn'
  title?: string
}>()

const conf = computed(() => {
  switch (props.kind || 'info') {
    case 'success':
      return { icon: CheckCircle2, cls: 'bg-mint-50 text-mint-700 border-mint-200' }
    case 'error':
      return { icon: XCircle, cls: 'bg-rose-50 text-rose-700 border-rose-200' }
    case 'warn':
      return { icon: AlertTriangle, cls: 'bg-amber-50 text-amber-700 border-amber-200' }
    default:
      return { icon: Info, cls: 'bg-sky-50 text-sky-700 border-sky-200' }
  }
})
</script>

<template>
  <div class="flex items-start gap-3 rounded-xl border px-4 py-3 text-sm" :class="conf.cls">
    <component :is="conf.icon" class="mt-0.5 h-4.5 w-4.5 shrink-0" />
    <div class="min-w-0">
      <p v-if="title" class="font-medium">{{ title }}</p>
      <div class="text-[13px] leading-relaxed opacity-90"><slot /></div>
    </div>
  </div>
</template>
