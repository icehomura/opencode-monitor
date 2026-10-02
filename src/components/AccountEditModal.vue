<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-mask" @click.self="$emit('close')">
      <div class="modal">
        <div class="modal-head">
          <div>
            <h2>编辑账号</h2>
            <p class="modal-sub">重命名账号，或将其设为主账号（标题栏与额度 / 模型卡片随之切换）</p>
          </div>
          <IconButton class="modal-close" title="关闭" @click="$emit('close')">
            <span style="font-size:15px">✕</span>
          </IconButton>
        </div>

        <div class="modal-body">
          <div class="form-col">
            <div class="form-row">
              <label class="form-label">名称</label>
              <BaseInput v-model="name" placeholder="账号名称" />
            </div>

            <div class="form-row">
              <label class="form-label">账号状态</label>
              <span class="ro-line">
                <span class="chip" :class="{ on: account?.logged_in }">{{ account?.logged_in ? '已登录' : '未登录' }}</span>
                <span v-if="account?.is_primary" class="chip primary">主账号</span>
                <span v-if="account?.org_id" class="ro-org" :title="account.org_id">{{ account.org_id }}</span>
              </span>
            </div>

            <div class="form-row">
              <label class="form-label">设为主账号</label>
              <span class="ro-line">
                <input type="checkbox" class="chk" v-model="setPrimary" :disabled="isPrimary" />
                <span class="chk-text">
                  <template v-if="isPrimary">当前已是主账号；如需更换，请在账号列表中把其他账号设为主账号</template>
                  <template v-else>设为后，标题栏与额度 / 模型卡片将显示该账号的数据</template>
                </span>
              </span>
            </div>

            <div class="form-row">
              <label class="form-label">本地数据</label>
              <span class="ro-line">
                <span class="ro-org">{{ account?.rows || 0 }} 条 · 最近同步 {{ fmtAgo(account?.last_sync_ms) }}</span>
              </span>
            </div>
          </div>
          <small class="err-hint">{{ errMsg }}</small>
        </div>

        <div class="modal-footer">
          <BaseButton @click="$emit('close')">取消</BaseButton>
          <BaseButton variant="primary" :disabled="!canSave" @click="submit">保存</BaseButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import IconButton from './base/IconButton.vue'
import BaseButton from './base/BaseButton.vue'
import BaseInput from './base/BaseInput.vue'
import { fmtAgo } from '../utils/format'

const props = defineProps({
  visible: { type: Boolean, default: false },
  account: { type: Object, default: null },
})
const emit = defineEmits(['save', 'close'])

const name = ref('')
const setPrimary = ref(false)
const errMsg = ref('')

const isPrimary = computed(() => !!props.account?.is_primary)
const canSave = computed(() => name.value.trim().length > 0)

watch(
  [() => props.visible, () => props.account],
  () => {
    if (!props.visible) return
    name.value = props.account?.name || ''
    setPrimary.value = !!props.account?.is_primary
    errMsg.value = ''
  },
  { immediate: true },
)

// 保存由父组件执行：rename_account +（勾选时）set_primary_account
function submit() {
  const n = name.value.trim()
  if (!n) { errMsg.value = '名称不能为空'; return }
  errMsg.value = ''
  emit('save', {
    id: props.account?.id || '',
    name: n,
    setPrimary: !!setPrimary.value && !isPrimary.value,
  })
}

function onKeydown(e) {
  if (e.key === 'Escape' && props.visible) emit('close')
}
onMounted(() => document.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
.modal-mask {
  position: fixed; inset: 0;
  background: rgba(5, 10, 20, .55);
  backdrop-filter: blur(3px);
  display: flex; align-items: center; justify-content: center;
  z-index: 110;
}
.modal {
  width: 460px; max-width: calc(100vw - 48px);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px 24px 14px;
  box-shadow: 0 12px 40px rgba(0,0,0,.35);
  max-height: calc(100vh - 64px);
  display: flex; flex-direction: column;
}
.modal-head { display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: 12px; flex-shrink: 0; }
.modal-head h2 { font-size: 16px; font-weight: 600; margin: 0; }
.modal-sub { font-size: 12px; color: var(--muted); margin: 2px 0 0; }
.modal-close { width: 34px !important; height: 34px !important; flex-shrink: 0; border-radius: 6px; }
.modal-body { overflow-y: auto; min-height: 0; }

.form-col { display: flex; flex-direction: column; gap: 12px; width: 100%; }
.form-row { display: flex; flex-direction: column; gap: 5px; }
.form-label { font-size: 12px; color: var(--muted); }

.ro-line { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; min-height: 22px; }
.ro-org { font-size: 11px; color: var(--muted); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chip {
  font-size: 11px; color: var(--muted); background: var(--border);
  border-radius: 10px; padding: 1px 8px; line-height: 1.6; white-space: nowrap;
}
.chip.on { color: var(--green); }
.chip.primary { color: var(--blue); background: rgba(79,140,255,.12); border: 1px solid rgba(79,140,255,.30); }
.chk { width: 15px; height: 15px; accent-color: var(--blue); cursor: pointer; flex-shrink: 0; }
.chk:disabled { cursor: not-allowed; opacity: .55; }
.chk-text { font-size: 11px; color: var(--muted); }

.err-hint { display: block; margin-top: 8px; min-height: 14px; font-size: 11px; color: #ff6b6b; }

.modal-footer {
  display: flex; align-items: center; justify-content: flex-end; gap: 16px;
  padding-top: 14px; margin-top: 12px; border-top: 1px solid var(--border); flex-shrink: 0;
}
</style>
