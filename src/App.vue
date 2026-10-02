<template>
  <div class="app-shell">
    <TitleBar
      :quota="stats.quota"
      @open-settings="showSettings = true"
    />
    <Toolbar
      :sync="stats.sync"
      :range="stats.range"
      :range-from="stats.rangeFrom"
      :range-to="stats.rangeTo"
      :login="stats.login"
      @range-change="onRangeChange"
      @sync-done="refresh()"
      @open-login="openLogin"
    />
    <section class="main-area">
      <StatsCards
        :quota="stats.quota"
        :reason="stats.quotaReason"
        :models="stats.models"
        :defaultModelId="stats.defaultModelId"
        :windowStats="stats.windowStats"
        :minute="stats.minute"
        :convertUnits="convertUnits"
      />
      <UsageChart
        :points="stats.series"
        :granularity="stats.granularity"
        :convertUnits="convertUnits"
      />
    </section>
    <SettingsModal
      :visible="showSettings"
      :themeName="themeName"
      :convertUnits="convertUnits"
      :login="stats.login"
      :auto-login="autoLogin"
      @close="showSettings = false"
      @update:themeName="setTheme"
      @update:convertUnits="v => { convertUnits = v; setConvertUnits(v) }"
      @changed="refresh()"
      @login-changed="refreshLogin"
    />
    <CloseDialog
      :visible="showCloseDialog"
      @close="showCloseDialog = false"
      @choice="onCloseChoice"
    />
  </div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue'
import TitleBar from './components/TitleBar.vue'
import Toolbar from './components/Toolbar.vue'
import StatsCards from './components/StatsCards.vue'
import UsageChart from './components/UsageChart.vue'
import SettingsModal from './components/SettingsModal.vue'
import CloseDialog from './components/CloseDialog.vue'
import { useMonitor } from './composables/useMonitor'
import { useTheme } from './composables/useTheme'
import { getConvertUnits, setConvertUnits } from './utils/format'
import { useTauri } from './composables/useTauri'

const { listen } = useTauri()
const { stats, refresh, refreshQuota, refreshSync, refreshLogin, loadModelsOnce } = useMonitor()
const { themeName, setTheme } = useTheme()

const showSettings = ref(false)
const showCloseDialog = ref(false)
const autoLogin = ref(false)
const convertUnits = ref(getConvertUnits())

function onRangeChange(range) { refresh(range) }

function openLogin() {
  showSettings.value = true
  autoLogin.value = true
  setTimeout(() => { autoLogin.value = false }, 800)
}

function onCloseChoice({ choice, remember }) {
  showCloseDialog.value = false
  try { window.__TAURI__.event?.emit('close-choice', { choice, remember }) } catch {}
}

let unlistenQuota = null
let unlistenSync = null
let unlistenClose = null
let pollTimer = null

onMounted(async () => {
  refresh()
  // 模型额度只在启动时取一次（等首次同步入库后）
  setTimeout(() => loadModelsOnce(), 4000)
  // 兜底轮询：即使事件丢失也能刷新
  pollTimer = setInterval(() => refresh(), 15000)
  unlistenQuota = await listen('quota-updated', refreshQuota)
  unlistenSync = await listen('sync-status', () => { refreshSync(); refresh() })
  unlistenClose = await listen('close-requested', () => { showCloseDialog.value = true })})

onBeforeUnmount(() => {
  unlistenQuota?.()
  unlistenSync?.()
  unlistenClose?.()
  clearInterval(pollTimer)
})
</script>

<style>
.main-area {
  padding: 16px;
  display: grid;
  grid-template-rows: auto 1fr;
  gap: 16px;
  height: calc(100vh - 40px - 46px);
  overflow: hidden;
}
</style>
