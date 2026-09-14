<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Search,
  Plus,
  ChevronUp,
  ChevronDown,
  Copy,
  Trash2,
  Pencil,
  Gamepad2,
  Layers,
  LayoutGrid,
  FolderCog,
} from 'lucide-vue-next'
import { store } from '@/store'
import type { Rule, RuleKind } from '@/types'
import {
  filterRules,
  moveRule,
  duplicateRule,
  type RuleEditDraft,
  emptyDraft,
  toDraft,
  draftToRule,
  draftIsValid,
} from '@/lib/rules'
import ToggleSwitch from '@/components/ui/ToggleSwitch.vue'
import RuleEditModal from '@/components/rules/RuleEditModal.vue'
import ProcessPickModal from '@/components/rules/ProcessPickModal.vue'
import TemplatePickModal from '@/components/rules/TemplatePickModal.vue'
import ConfirmModal from '@/components/ui/ConfirmModal.vue'

const st = store.state

const query = ref('')
const kindFilter = ref<'all' | RuleKind>('all')

const rules = computed<Rule[]>(() => st.config?.rules ?? [])
const visible = computed(() => filterRules(rules.value, query.value, kindFilter.value))

const gameCount = computed(() => rules.value.filter((r) => r.kind === 'game').length)
const platformCount = computed(() => rules.value.filter((r) => r.kind === 'platform').length)
const activeCount = computed(() => rules.value.filter((r) => r.enabled).length)

/* ---- 编辑弹窗 ---- */
const editOpen = ref(false)
const draft = ref<RuleEditDraft>(emptyDraft())
const editingId = ref<string | null>(null) // null = 新建

/* 进程选择 / EXE 选择由 RuleEditModal 内触发，通过事件回填 */
const pickOpen = ref(false)
const pickTarget = ref<{ index: number } | null>(null)

function openCreate(kind: RuleKind = 'game') {
  draft.value = emptyDraft(kind)
  editingId.value = null
  editOpen.value = true
}

function openEdit(rule: Rule) {
  draft.value = toDraft(rule)
  editingId.value = rule.id
  editOpen.value = true
}

function onSaveEdit() {
  if (!draftIsValid(draft.value)) return
  const rule = draftToRule(draft.value)
  store.mutateConfig((c) => {
    const idx = c.rules.findIndex((r) => r.id === rule.id)
    if (idx === -1) c.rules.push(rule)
    else c.rules.splice(idx, 1, rule)
  })
  editOpen.value = false
}

/* ---- 复制 ---- */
function onDuplicate(id: string) {
  const out = duplicateRule(rules.value, id)
  if (!out) return
  store.mutateConfig((c) => {
    c.rules = out.rules
  })
}

/* ---- 删除确认 ---- */
const confirmOpen = ref(false)
const deleteTarget = ref<Rule | null>(null)
function askDelete(rule: Rule) {
  deleteTarget.value = rule
  confirmOpen.value = true
}
function doDelete() {
  const t = deleteTarget.value
  if (!t) return
  store.mutateConfig((c) => {
    c.rules = c.rules.filter((r) => r.id !== t.id)
  })
  confirmOpen.value = false
  deleteTarget.value = null
}

/* ---- 开关 / 排序 ---- */
function toggleRule(rule: Rule) {
  store.mutateConfig((c) => {
    const r = c.rules.find((x) => x.id === rule.id)
    if (r) r.enabled = !r.enabled
  })
}

function doMove(id: string, dir: -1 | 1) {
  const next = moveRule(rules.value, id, dir)
  store.mutateConfig((c) => {
    c.rules = next
  })
}

/* ---- 模板 ---- */
const templateOpen = ref(false)
</script>

<template>
  <div class="mx-auto max-w-[1020px] px-8 py-6">
    <!-- 工具栏 -->
    <div class="mb-5 flex flex-wrap items-center gap-3">
      <div class="relative min-w-[220px] flex-1">
        <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
        <input
          v-model="query"
          type="text"
          placeholder="搜索规则名称或进程…"
          class="w-full rounded-xl border border-slate-200 bg-white py-2.5 pl-9 pr-3 text-sm shadow-card transition focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
        />
      </div>

      <div class="flex rounded-xl border border-slate-200 bg-white p-1 shadow-card">
        <button
          v-for="f in [
            { id: 'all', label: '全部', icon: LayoutGrid },
            { id: 'game', label: '游戏', icon: Gamepad2 },
            { id: 'platform', label: '平台', icon: Layers },
          ] as const"
          :key="f.id"
          class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[13px] font-medium transition"
          :class="
            kindFilter === f.id ? 'bg-mint-600 text-white shadow-sm' : 'text-slate-600 hover:bg-slate-50'
          "
          @click="kindFilter = f.id"
        >
          <component :is="f.icon" class="h-3.5 w-3.5" />
          {{ f.label }}
        </button>
      </div>

      <button
        class="flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3.5 py-2.5 text-[13px] font-medium text-slate-700 shadow-card transition hover:bg-slate-50"
        @click="templateOpen = true"
      >
        <FolderCog class="h-4 w-4 text-slate-500" />
        平台模板
      </button>

      <button
        class="flex items-center gap-1.5 rounded-xl bg-mint-600 px-4 py-2.5 text-sm font-medium text-white shadow-sm transition hover:bg-mint-700"
        @click="openCreate('game')"
      >
        <Plus class="h-4 w-4" />
        新建规则
      </button>
    </div>

    <!-- 规则列表 -->
    <div v-if="visible.length === 0" class="rounded-2xl bg-white p-12 text-center shadow-card">
      <p class="text-slate-500">
        {{ rules.length === 0 ? '还没有规则。点击「新建规则」或从平台模板快速创建。' : '没有匹配的规则。' }}
      </p>
    </div>

    <TransitionGroup v-else name="rule-list" tag="div" class="space-y-3">
      <article
        v-for="(rule, i) in visible"
        :key="rule.id"
        class="group flex items-center gap-4 rounded-2xl bg-white p-4 shadow-card transition-shadow hover:shadow-card-hover"
        :class="{ 'opacity-60': !rule.enabled }"
      >
        <!-- 序号/序位 -->
        <div class="flex w-6 shrink-0 flex-col items-center">
          <span class="text-[11px] font-semibold tabular-nums text-slate-300">{{ i + 1 }}</span>
        </div>

        <!-- 主体 -->
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span
              class="rounded-md px-1.5 py-0.5 text-[11px] font-semibold"
              :class="rule.kind === 'game' ? 'bg-sky-50 text-sky-600' : 'bg-violet-50 text-violet-600'"
            >
              {{ rule.kind === 'game' ? '游戏' : '平台' }}
            </span>
            <h3 class="truncate text-[15px] font-semibold text-slate-900">{{ rule.name }}</h3>
            <span
              v-if="st.status?.activeRuleId === rule.id"
              class="rounded-full bg-mint-50 px-2 py-0.5 text-[11px] font-medium text-mint-700"
            >
              当前命中
            </span>
          </div>
          <p class="mt-1 truncate text-[13px] text-slate-400">
            {{ rule.processes.map((p) => p.name).join(' · ') }}
          </p>
        </div>

        <!-- 音量 -->
        <div class="shrink-0 text-right">
          <p class="text-lg font-bold tabular-nums" :class="rule.enabled ? 'text-mint-700' : 'text-slate-400'">
            {{ rule.volume }}<span class="text-xs font-medium">%</span>
          </p>
          <p class="text-[11px] text-slate-400">目标音量</p>
        </div>

        <!-- 操作 -->
        <div class="flex shrink-0 items-center gap-1">
          <div class="flex flex-col">
            <button
              class="rounded p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 disabled:opacity-25"
              :disabled="rules.findIndex(r => r.id === rule.id) === 0 || !!query || kindFilter !== 'all'"
              aria-label="上移"
              @click="doMove(rule.id, -1)"
            >
              <ChevronUp class="h-3.5 w-3.5" />
            </button>
            <button
              class="rounded p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 disabled:opacity-25"
              :disabled="rules.findIndex(r => r.id === rule.id) === rules.length - 1 || !!query || kindFilter !== 'all'"
              aria-label="下移"
              @click="doMove(rule.id, 1)"
            >
              <ChevronDown class="h-3.5 w-3.5" />
            </button>
          </div>

          <ToggleSwitch :model-value="rule.enabled" :label="`启用 ${rule.name}`" @update:model-value="toggleRule(rule)" />

          <span class="mx-1 h-6 w-px bg-slate-100"></span>

          <button
            class="rounded-lg p-2 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700"
            aria-label="编辑"
            title="编辑"
            @click="openEdit(rule)"
          >
            <Pencil class="h-4 w-4" />
          </button>
          <button
            class="rounded-lg p-2 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700"
            aria-label="复制"
            title="复制"
            @click="onDuplicate(rule.id)"
          >
            <Copy class="h-4 w-4" />
          </button>
          <button
            class="rounded-lg p-2 text-slate-400 transition hover:bg-rose-50 hover:text-rose-600"
            aria-label="删除"
            title="删除"
            @click="askDelete(rule)"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </article>
    </TransitionGroup>

    <!-- 底部统计 -->
    <p v-if="rules.length" class="mt-4 text-center text-xs text-slate-400">
      共 {{ rules.length }} 条规则 · 游戏 {{ gameCount }} / 平台 {{ platformCount }} · 启用 {{ activeCount }} ·
      顺序即优先级（游戏优先于平台）
    </p>

    <!-- 弹窗们 -->
    <RuleEditModal
      v-model:open="editOpen"
      :draft="draft"
      :rules="rules"
      :editing="!!editingId"
      @save="onSaveEdit"
      @pick-process="pickTarget = $event; pickOpen = true"
    />

    <ProcessPickModal
      v-model:open="pickOpen"
      @select="
        (p) => {
          if (pickTarget && draft.processes[pickTarget.index]) {
            draft.processes[pickTarget.index] = { name: p.name, path: p.path }
          }
        }
      "
    />

    <TemplatePickModal v-model:open="templateOpen" />

    <ConfirmModal
      v-model:open="confirmOpen"
      title="删除规则"
      :message="`确定删除规则「${deleteTarget?.name ?? ''}」？此操作保存配置后生效，删除后无法撤销。`"
      confirm-text="删除"
      danger
      @confirm="doDelete"
    />
  </div>
</template>

<style scoped>
.rule-list-move {
  transition: transform 0.2s ease;
}
.rule-list-enter-active,
.rule-list-leave-active {
  transition: opacity 0.15s ease;
}
.rule-list-enter-from,
.rule-list-leave-to {
  opacity: 0;
}
</style>
