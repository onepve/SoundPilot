<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { X } from 'lucide-vue-next'

const props = defineProps<{ open: boolean; title: string; subtitle?: string; wide?: boolean }>()
const emit = defineEmits<{ (e: 'close'): void }>()
const panel = ref<HTMLElement>()
let previous: HTMLElement | null = null
function keydown(e: KeyboardEvent) {
  const dialogs = document.querySelectorAll('[role="dialog"]')
  if (!props.open || dialogs[dialogs.length - 1] !== panel.value) return
  if (e.key === 'Escape') { e.stopImmediatePropagation(); emit('close') }
  if (e.key === 'Tab') {
    const focusable = Array.from(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]') ?? [])
      .filter(el => el.getClientRects().length)
    const first = focusable[0], last = focusable[focusable.length - 1]
    if (!first) { e.preventDefault(); return }
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus() }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus() }
  }
}
watch(() => props.open, async open => {
  if (open) {
    previous = document.activeElement as HTMLElement
    await nextTick()
    panel.value?.focus()
    document.addEventListener('keydown', keydown)
  } else {
    document.removeEventListener('keydown', keydown)
    previous?.focus()
  }
}, { immediate: true })
onBeforeUnmount(() => document.removeEventListener('keydown', keydown))
</script>

<template>
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/35 p-6 backdrop-blur-[2px]" @mousedown.self="emit('close')">
        <div ref="panel" role="dialog" aria-modal="true" :aria-label="title" tabindex="-1"
          class="flex max-h-[86vh] w-full flex-col overflow-hidden rounded-2xl bg-white shadow-pop outline-none"
          :class="wide ? 'max-w-3xl' : 'max-w-lg'">
          <header class="flex shrink-0 items-start justify-between border-b border-slate-100 px-6 py-4">
            <div><h2 class="text-lg font-semibold text-slate-900">{{ title }}</h2><p v-if="subtitle" class="mt-0.5 text-sm text-slate-500">{{ subtitle }}</p></div>
            <button type="button" class="rounded-lg p-1.5 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600" aria-label="关闭" @click="emit('close')"><X class="h-5 w-5" /></button>
          </header>
          <div class="min-h-0 flex-1 overflow-y-auto px-6 py-5"><slot /></div>
          <footer v-if="$slots.footer" class="shrink-0 border-t border-slate-100 bg-slate-50/60 px-6 py-4"><slot name="footer" /></footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
<style scoped>
.modal-fade-enter-active,.modal-fade-leave-active { transition: opacity .15s ease; }
.modal-fade-enter-from,.modal-fade-leave-to { opacity: 0; }
</style>
