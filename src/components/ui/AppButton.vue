<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
  disabled?: boolean
  loading?: boolean
  block?: boolean
}>()

const variantCls = computed(() => {
  switch (props.variant || 'primary') {
    case 'secondary':
      return 'bg-white text-slate-700 border border-slate-300 hover:bg-slate-50 active:bg-slate-100'
    case 'ghost':
      return 'text-slate-600 hover:bg-slate-100 active:bg-slate-200'
    case 'danger':
      return 'bg-rose-600 text-white hover:bg-rose-700 active:bg-rose-800 shadow-sm'
    default:
      return 'bg-mint-600 text-white hover:bg-mint-700 active:bg-mint-800 shadow-sm'
  }
})
</script>

<template>
  <button
    type="button"
    :disabled="disabled || loading"
    class="inline-flex select-none items-center justify-center gap-1.5 rounded-lg font-medium transition-colors duration-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-mint-500/50 disabled:cursor-not-allowed disabled:opacity-45"
    :class="[
      variantCls,
      size === 'sm' ? 'px-2.5 py-1.5 text-[13px]' : 'px-4 py-2 text-sm',
      block ? 'w-full' : '',
    ]"
  >
    <span
      v-if="loading"
      class="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent"
    />
    <slot />
  </button>
</template>
