<script setup lang="ts">
import { AlertTriangle } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import AppButton from '@/components/ui/AppButton.vue'

defineProps<{
  open: boolean
  title: string
  message: string
  confirmText?: string
  danger?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'confirm'): void
}>()
</script>

<template>
  <AppModal :open="open" :title="title" @close="emit('update:open', false)">
    <div class="flex items-start gap-3.5">
      <div
        class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full"
        :class="danger ? 'bg-rose-50' : 'bg-amber-50'"
      >
        <AlertTriangle class="h-5 w-5" :class="danger ? 'text-rose-500' : 'text-amber-500'" />
      </div>
      <p class="pt-1.5 text-sm leading-relaxed text-slate-600">{{ message }}</p>
    </div>
    <template #footer>
      <div class="flex justify-end gap-2.5">
        <AppButton variant="secondary" @click="emit('update:open', false)">取消</AppButton>
        <AppButton :variant="danger ? 'danger' : 'primary'" @click="emit('confirm')">
          {{ confirmText ?? '确定' }}
        </AppButton>
      </div>
    </template>
  </AppModal>
</template>
