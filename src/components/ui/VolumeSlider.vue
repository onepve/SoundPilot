<script setup lang="ts">
defineProps<{
  label: string
  suffix?: string
  min?: number
  max?: number
  step?: number
  disabled?: boolean
}>()
const model = defineModel<number>({ required: true })
</script>

<template>
  <div>
    <div class="mb-1.5 flex items-baseline justify-between">
      <label class="text-sm font-medium text-slate-700">{{ label }}</label>
      <input
        v-model.number="model"
        type="number"
        :min="min"
        :max="max"
        :step="step || 1"
        :disabled="disabled"
        class="w-16 rounded-md border border-slate-200 bg-white px-2 py-0.5 text-right text-sm tabular-nums text-slate-800 focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500 disabled:bg-slate-50 disabled:text-slate-400"
      />
    </div>
    <input
      v-model.number="model"
      type="range"
      :min="min ?? 0"
      :max="max ?? 100"
      :step="step ?? 1"
      :disabled="disabled"
      class="slider-mint w-full"
    />
  </div>
</template>
