<script setup lang="ts">
import { computed } from 'vue'
import { Plus, Trash2, Crosshair, FolderOpen, MonitorSmartphone, AlertTriangle } from 'lucide-vue-next'
import type { Rule } from '@/types'
import { validateDraft, findProcessConflicts, type RuleEditDraft } from '@/lib/rules'
import { bridge, hasBridge } from '@/lib/bridge'
import { store } from '@/store'
import AppModal from '@/components/ui/AppModal.vue'
import AppButton from '@/components/ui/AppButton.vue'
import AppBanner from '@/components/ui/AppBanner.vue'
import VolumeSlider from '@/components/ui/VolumeSlider.vue'
import ToggleSwitch from '@/components/ui/ToggleSwitch.vue'

const props = defineProps<{
  open: boolean
  draft: RuleEditDraft
  rules: Rule[]
  editing: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'save'): void
  (e: 'pick-process', target: { index: number }): void
}>()

const issues = computed(() => validateDraft(props.draft))
const valid = computed(
  () => issues.value.name === null && issues.value.volume === null && issues.value.processes.every((x) => x === ''),
)

const conflicts = computed(() =>
  findProcessConflicts(props.rules, props.draft.id),
)

function close() {
  emit('update:open', false)
}

function trySave() {
  if (!valid.value) return
  emit('save')
}

function addProcess() {
  props.draft.processes.push({ name: '', path: '' })
}

function removeProcess(i: number) {
  props.draft.processes.splice(i, 1)
  if (props.draft.processes.length === 0) props.draft.processes.push({ name: '', path: '' })
}

/** EXE 文件选择：Rust 原生对话框 pick_executable */
async function pickExe(i: number) {
  if (!hasBridge()) {
    store.notify('error', '选择 EXE 文件需要桌面环境')
    return
  }
  try {
    const picked = await bridge.pickExecutable()
    if (picked) {
      props.draft.processes[i] = { name: picked.name, path: picked.path }
    }
  } catch (err) {
    store.notify('error', `选择文件失败：${String((err as Error)?.message ?? err)}`)
  }
}
</script>

<template>
  <AppModal
    :open="open"
    :title="editing ? '编辑规则' : '新建规则'"
    subtitle="命中进程后，启用中的音箱将调整到目标音量"
    wide
    @close="close"
  >
    <div class="space-y-5">
      <!-- 基本信息 -->
      <div class="grid grid-cols-2 gap-4">
        <label class="block">
          <span class="mb-1.5 block text-sm font-medium text-slate-700">规则名称 <span class="text-rose-500">*</span></span>
          <input
            v-model.trim="draft.name"
            type="text"
            placeholder="例如：英雄联盟"
            class="w-full rounded-lg border px-3 py-2 text-sm transition focus:outline-none focus:ring-1"
            :class="
              issues.name
                ? 'border-rose-300 focus:border-rose-500 focus:ring-rose-500'
                : 'border-slate-200 focus:border-mint-500 focus:ring-mint-500'
            "
          />
          <p v-if="issues.name" class="mt-1 text-xs text-rose-500">{{ issues.name }}</p>
        </label>

        <div>
          <span class="mb-1.5 block text-sm font-medium text-slate-700">类型</span>
          <div class="flex rounded-lg border border-slate-200 p-1">
            <button
              v-for="k in [
                { id: 'game', label: '游戏', icon: MonitorSmartphone },
                { id: 'platform', label: '平台', icon: Crosshair },
              ] as const"
              :key="k.id"
              class="flex flex-1 items-center justify-center gap-1.5 rounded-md py-1.5 text-[13px] font-medium transition"
              :class="draft.kind === k.id ? 'bg-mint-600 text-white shadow-sm' : 'text-slate-600 hover:bg-slate-50'"
              @click="draft.kind = k.id"
            >
              <component :is="k.icon" class="h-3.5 w-3.5" />
              {{ k.label }}
            </button>
          </div>
        </div>
      </div>

      <!-- 音量 -->
      <div class="rounded-xl border border-slate-100 bg-slate-50/60 px-4 py-3.5">
        <VolumeSlider v-model="draft.volume" label="目标音量" suffix="%" :min="0" :max="100" />
        <p v-if="issues.volume" class="mt-1 text-xs text-rose-500">{{ issues.volume }}</p>
      </div>

      <!-- 启用 -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-slate-700">启用此规则</p>
          <p class="text-xs text-slate-400">关闭后保留配置但不参与匹配</p>
        </div>
        <ToggleSwitch v-model="draft.enabled" label="启用此规则" />
      </div>

      <!-- 进程列表 -->
      <div>
        <div class="mb-2 flex items-center justify-between">
          <span class="text-sm font-medium text-slate-700">
            关联进程 <span class="text-rose-500">*</span>
            <span class="ml-1 text-xs font-normal text-slate-400">（一条规则可关联多个进程，任一运行即命中）</span>
          </span>
          <AppButton size="sm" variant="secondary" @click="addProcess">
            <Plus class="h-3.5 w-3.5" /> 添加进程
          </AppButton>
        </div>

        <div class="space-y-2.5">
          <div
            v-for="(p, i) in draft.processes"
            :key="i"
            class="flex items-start gap-2 rounded-xl border bg-white p-3"
            :class="issues.processes[i] ? 'border-rose-200' : 'border-slate-200'"
          >
            <span class="mt-2 w-5 shrink-0 text-center text-xs font-semibold tabular-nums text-slate-300">
              {{ i + 1 }}
            </span>

            <div class="min-w-0 flex-1 space-y-2">
              <div class="flex items-center gap-2">
                <input
                  v-model.trim="p.name"
                  type="text"
                  :placeholder="`进程名（如 game.exe，不含路径时大小写不敏感）`"
                  class="min-w-0 flex-1 rounded-lg border border-slate-200 px-2.5 py-1.5 font-mono text-[13px] focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
                />
                <button
                  type="button"
                  class="shrink-0 rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs font-medium text-slate-600 transition hover:bg-slate-50"
                  title="从正在运行的进程中选择"
                  @click="emit('pick-process', { index: i })"
                >
                  <Crosshair class="mr-1 inline h-3.5 w-3.5" />运行程序
                </button>
              </div>
              <div class="flex items-center gap-2">
                <input
                  v-model.trim="p.path"
                  type="text"
                  placeholder="完整路径（可选；填写后按路径精准匹配，如 D:\\Games\\lol\\LeagueClient.exe）"
                  class="min-w-0 flex-1 rounded-lg border border-slate-200 bg-slate-50/60 px-2.5 py-1.5 font-mono text-xs text-slate-600 focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
                />
                <button
                  type="button"
                  class="shrink-0 rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs font-medium text-slate-600 transition hover:bg-slate-50"
                  title="浏览选择 EXE 文件"
                  @click="pickExe(i)"
                >
                  <FolderOpen class="mr-1 inline h-3.5 w-3.5" />选择 EXE
                </button>
              </div>
            </div>

            <button
              type="button"
              class="mt-1 shrink-0 rounded-lg p-1.5 text-slate-300 transition hover:bg-rose-50 hover:text-rose-500 disabled:opacity-30"
              :disabled="draft.processes.length <= 1"
              :aria-label="`移除进程 ${i + 1}`"
              @click="removeProcess(i)"
            >
              <Trash2 class="h-4 w-4" />
            </button>
          </div>
        </div>

        <p v-for="(err, i) in issues.processes" v-show="err" :key="'e' + i" class="mt-1 text-xs text-rose-500">
          进程 {{ i + 1 }}：{{ err }}
        </p>
      </div>

      <!-- 冲突提示 -->
      <AppBanner v-if="conflicts.length" kind="warn" title="检测到进程重叠">
        以下启用中的规则关联了相同进程，按列表顺序先命中者优先：
        <ul class="mt-1 list-inside list-disc">
          <li v-for="c in conflicts" :key="c">{{ c }}</li>
        </ul>
      </AppBanner>

      <AppBanner kind="info">
        匹配规则：游戏优先于平台；同组内按列表顺序（或最高音量，见设置）取第一条命中的规则。
      </AppBanner>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2.5">
        <AppButton variant="secondary" @click="close">取消</AppButton>
        <AppButton :disabled="!valid" @click="trySave">
          <AlertTriangle v-if="!valid" class="h-4 w-4" />
          {{ editing ? '保存修改' : '创建规则' }}
        </AppButton>
      </div>
    </template>
  </AppModal>
</template>
