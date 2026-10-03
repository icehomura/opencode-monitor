<template>
  <Teleport to="body">
    <div v-if="visible" class="close-dialog-mask" @click.self="onCancel">
      <div class="close-dialog">
        <div class="close-dialog-icon">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
        </div>
        <h3 class="close-dialog-title">关闭 Usage Monitor？</h3>
        <p class="close-dialog-desc">关闭后后台的额度刷新与日志同步将暂停</p>

        <div class="close-dialog-options">
          <label
            class="close-option"
            :class="{ active: selected === 'minimize' }"
            @click="selected = 'minimize'"
          >
            <span class="close-option-icon">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <path d="M5 12h14" />
              </svg>
            </span>
            <span class="close-option-text">
              <span class="close-option-label">最小化到托盘</span>
              <span class="close-option-hint">后台同步继续运行</span>
            </span>
            <span v-if="selected === 'minimize'" class="close-option-check">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
            </span>
          </label>
          <label
            class="close-option close-option--danger"
            :class="{ active: selected === 'quit' }"
            @click="selected = 'quit'"
          >
            <span class="close-option-icon">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
                <polyline points="16 17 21 12 16 7" />
                <line x1="21" y1="12" x2="9" y2="12" />
              </svg>
            </span>
            <span class="close-option-text">
              <span class="close-option-label">关闭程序</span>
              <span class="close-option-hint">停止同步并退出</span>
            </span>
            <span v-if="selected === 'quit'" class="close-option-check">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
            </span>
          </label>
        </div>

        <label class="close-remember">
          <input type="checkbox" v-model="remember" class="close-remember-cb" />
          <span>记住我的选择</span>
          <span class="close-remember-hint">（可在设置中修改）</span>
        </label>

        <div class="close-dialog-actions">
          <button class="close-btn close-btn--cancel" @click="onCancel">取消</button>
          <button class="close-btn close-btn--confirm" @click="onConfirm">确认</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref } from 'vue'

const props = defineProps({
  visible: { type: Boolean, default: false },
})

const emit = defineEmits(['close', 'choice'])

const selected = ref('minimize')
const remember = ref(false)

function onConfirm() {
  emit('choice', { choice: selected.value, remember: remember.value })
}

function onCancel() {
  emit('close')
}
</script>

<style scoped>
.close-dialog-mask {
  position: fixed; inset: 0;
  background: rgba(5, 10, 20, .6);
  backdrop-filter: blur(4px);
  display: flex; align-items: center; justify-content: center;
  z-index: 200;
  animation: fadeIn .15s ease;
}
@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }

.close-dialog {
  width: 400px; max-width: calc(100vw - 48px);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 28px 24px 20px;
  box-shadow: 0 16px 48px rgba(0,0,0,.45);
  text-align: center;
  animation: slideUp .2s ease;
}
@keyframes slideUp { from { transform: translateY(12px); opacity: 0; } to { transform: translateY(0); opacity: 1; } }

.close-dialog-icon {
  display: inline-flex; align-items: center; justify-content: center;
  width: 48px; height: 48px; border-radius: 50%;
  background: rgba(255, 107, 107, .12);
  color: #ff6b6b;
  margin-bottom: 14px;
}
.close-dialog-title {
  font-size: 16px; font-weight: 600; color: var(--text); margin: 0 0 6px;
}
.close-dialog-desc {
  font-size: 12px; color: var(--muted); margin: 0 0 18px;
}

.close-dialog-options {
  display: flex; flex-direction: column; gap: 8px;
  margin-bottom: 14px;
}
.close-option {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 14px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  cursor: pointer;
  transition: border-color .15s, background .15s;
}
.close-option:hover { border-color: var(--muted); }
.close-option.active { border-color: var(--blue); background: rgba(79, 140, 255, .06); }
.close-option--danger.active { border-color: #ff6b6b; background: rgba(255, 107, 107, .06); }
.close-option-icon {
  display: flex; align-items: center; justify-content: center;
  width: 28px; height: 28px; border-radius: 6px;
  background: var(--border); color: var(--muted); flex-shrink: 0;
}
.close-option.active .close-option-icon { background: rgba(79, 140, 255, .15); color: var(--blue); }
.close-option--danger.active .close-option-icon { background: rgba(255, 107, 107, .15); color: #ff6b6b; }
.close-option-text { flex: 1; text-align: left; }
.close-option-label { display: block; font-size: 13px; font-weight: 600; color: var(--text); }
.close-option-hint { display: block; font-size: 11px; color: var(--muted); margin-top: 1px; }
.close-option-check { color: var(--blue); flex-shrink: 0; }
.close-option--danger.active .close-option-check { color: #ff6b6b; }

.close-remember {
  display: flex; align-items: center; justify-content: center; gap: 6px;
  font-size: 12px; color: var(--muted);
  margin-bottom: 18px; cursor: pointer;
}
.close-remember-cb {
  width: 14px; height: 14px; accent-color: var(--blue); cursor: pointer;
}
.close-remember-hint { font-size: 11px; opacity: .7; }

.close-dialog-actions {
  display: flex; gap: 10px; justify-content: center;
}
.close-btn {
  padding: 7px 24px; border-radius: 6px;
  font-size: 13px; font-weight: 500; cursor: pointer;
  border: 1px solid transparent; transition: filter .1s;
}
.close-btn--cancel {
  background: var(--border); color: var(--text);
}
.close-btn--cancel:hover { filter: brightness(1.2); }
.close-btn--confirm {
  background: var(--blue); color: #fff;
}
.close-btn--confirm:hover { filter: brightness(1.15); }
</style>
