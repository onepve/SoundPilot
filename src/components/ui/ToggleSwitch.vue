<script setup lang="ts">
const props = defineProps<{
  modelValue: boolean
  label?: string
  disabled?: boolean
  small?: boolean
}>()

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

function flip() {
  if (!props.disabled) emit('update:modelValue', !props.modelValue)
}
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="label"
    :disabled="disabled"
    class="relative inline-flex shrink-0 items-center rounded-full transition-colors duration-150 focus:outline-none focus-visible:ring-2 focus-visible:ring-mint-500/60 disabled:cursor-not-allowed disabled:opacity-40"
    :class="[small ? 'h-[18px] w-8' : 'h-6 w-11', modelValue ? 'bg-mint-500' : 'bg-slate-300']"
    @click="flip"
  >
    <span
      class="absolute top-0.5 left-0.5 rounded-full bg-white shadow transition-transform duration-150"
      :class="small ? 'h-3.5 w-3.5' : 'h-5 w-5'"
      :style="{
        transform: modelValue ? (small ? 'translateX(14px)' : 'translateX(20px)') : 'translateX(0)',
      }"
    />
  </button>
</template>
