<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Home, Gamepad2, Settings, Volume2, WifiOff, FlaskConical } from 'lucide-vue-next'
import { store } from '@/store'
import HomePage from '@/pages/HomePage.vue'
import RulesPage from '@/pages/RulesPage.vue'
import SettingsPage from '@/pages/SettingsPage.vue'

type PageId = 'home' | 'rules' | 'settings'

const page = ref<PageId>('home')

const pages: { id: PageId; label: string; desc: string; icon: typeof Home }[] = [
  { id: 'home', label: '首页', desc: '运行状态与快捷操作', icon: Home },
  { id: 'rules', label: '游戏与平台', desc: '音量规则管理', icon: Gamepad2 },
  { id: 'settings', label: '设置', desc: '连接、音箱与偏好', icon: Settings },
]

const current = computed(() => pages.find((p) => p.id === page.value)!)

const st = store.state

const connLabel = computed(() => {
  if (st.demo) return '演示模式 · 未连接'
  if (st.offline) return '浏览器预览'
  return st.status?.connection ?? '…'
})

onMounted(() => {
  store.init()
})
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden">
    <!-- 侧栏 -->
    <aside class="flex w-[228px] shrink-0 flex-col border-r border-slate-200 bg-white">
      <div class="flex items-center gap-3 px-5 pb-5 pt-6">
        <div
          class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-mint-500 to-teal-600 shadow-sm"
        >
          <Volume2 class="h-5.5 w-5.5 text-white" />
        </div>
        <div>
          <h1 class="text-[17px] font-bold leading-tight text-slate-900">SoundPilot</h1>
          <p class="text-[11px] text-slate-400">小爱音量助手</p>
        </div>
      </div>

      <nav class="flex-1 space-y-1 px-3">
        <button
          v-for="p in pages"
          :key="p.id"
          class="group flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors"
          :class="
            page === p.id
              ? 'bg-mint-50 text-mint-800'
              : 'text-slate-600 hover:bg-slate-50 hover:text-slate-900'
          "
          @click="page = p.id"
        >
          <component
            :is="p.icon"
            class="h-5 w-5 shrink-0"
            :class="page === p.id ? 'text-mint-600' : 'text-slate-400 group-hover:text-slate-600'"
          />
          <span class="min-w-0">
            <span class="block text-sm font-medium leading-tight">{{ p.label }}</span>
            <span class="block truncate text-[11px] text-slate-400">{{ p.desc }}</span>
          </span>
        </button>
      </nav>

      <!-- 底部状态 -->
      <div class="mx-3 mb-4 rounded-xl bg-slate-50 px-3.5 py-3">
        <div class="flex items-center gap-2">
          <span
            class="h-2 w-2 shrink-0 rounded-full"
            :class="st.offline ? 'bg-slate-400' : 'bg-mint-500'"
          />
          <span class="truncate text-xs font-medium text-slate-600">{{ connLabel }}</span>
        </div>
        <p v-if="st.offline" class="mt-1.5 text-[11px] leading-snug text-slate-400">
          未检测到桌面环境，仅界面预览
        </p>
        <p v-else-if="st.status" class="mt-1.5 text-[11px] text-slate-400">
          v{{ st.status.version }} · {{ st.status.paused ? '已暂停' : '监控中' }}
        </p>
      </div>
    </aside>

    <!-- 主区域 -->
    <main class="relative flex min-w-0 flex-1 flex-col bg-slate-100">
      <!-- 离线 / 演示横幅 -->
      <div
        v-if="st.offline && !st.demo"
        class="flex items-center gap-2 border-b border-amber-200 bg-amber-50 px-6 py-2 text-[13px] text-amber-800"
      >
        <WifiOff class="h-4 w-4 shrink-0" />
        <span
          >当前在普通浏览器中运行，桌面功能不可用：无法读写配置、检测进程或连接音箱。请通过安装的
          SoundPilot 桌面程序使用完整功能。</span
        >
      </div>
      <div
        v-else-if="st.demo"
        class="flex items-center gap-2 border-b border-violet-200 bg-violet-50 px-6 py-2 text-[13px] text-violet-800"
      >
        <FlaskConical class="h-4 w-4 shrink-0" />
        <span
          >演示模式（开发测试用）：以下为本地虚构数据，不会连接任何音箱或服务，配置不保存。正式功能请使用桌面程序。</span
        >
      </div>

      <header
        class="flex items-center justify-between border-b border-slate-200 bg-white/70 px-8 py-4 backdrop-blur"
      >
        <div>
          <h2 class="text-xl font-bold text-slate-900">{{ current.label }}</h2>
          <p class="text-[13px] text-slate-400">{{ current.desc }}</p>
        </div>
        <div class="flex items-center gap-2">
          <span
            v-if="st.dirty"
            class="rounded-full bg-amber-100 px-3 py-1 text-xs font-medium text-amber-700"
          >
            有未保存的修改
          </span>
          <button
            v-if="!st.offline && st.dirty"
            class="rounded-lg bg-mint-600 px-4 py-1.5 text-sm font-medium text-white shadow-sm transition hover:bg-mint-700"
            :disabled="st.saving"
            @click="store.save()"
          >
            {{ st.saving ? '保存中…' : '保存配置' }}
          </button>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto">
        <Transition name="fade-slide" mode="out-in">
          <HomePage v-if="page === 'home'" key="home" />
          <RulesPage v-else-if="page === 'rules'" key="rules" />
          <SettingsPage v-else key="settings" />
        </Transition>
      </div>
    </main>

    <!-- Toasts -->
    <div class="pointer-events-none fixed bottom-6 right-6 z-[60] flex flex-col items-end gap-2">
      <TransitionGroup name="toast">
        <div
          v-for="t in st.toasts"
          :key="t.id"
          class="pointer-events-auto flex items-center gap-2 rounded-xl px-4 py-2.5 text-sm text-white shadow-pop"
          :class="t.kind === 'error' ? 'bg-rose-600' : t.kind === 'success' ? 'bg-mint-600' : 'bg-slate-700'"
        >
          {{ t.message }}
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.18s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
