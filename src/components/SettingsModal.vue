<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-mask" @click.self="$emit('close')">
      <div class="modal">
        <div class="modal-head">
          <div>
            <h2>设置</h2>
            <p class="modal-sub">OpenCode 账户、日志同步与系统设置</p>
          </div>
          <IconButton class="modal-close" title="关闭" @click="$emit('close')">
            <span style="font-size:15px">✕</span>
          </IconButton>
        </div>

        <div class="tabs">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            class="tab-btn"
            :class="{ active: activeTab === tab.id }"
            @click="switchTab(tab.id)"
          >{{ tab.label }}</button>
        </div>

        <div class="modal-body">
        <div class="tab-content">

        <!-- ── Tab：账户 ── -->
        <div v-show="activeTab === 'account'" class="tab-pane">
          <SettingsCard title="OpenCode 账户" description="填 API Key 或直接登录授权，任一即可；均自动保存">
            <div class="form-col">
              <div class="settings-row">
                <span class="settings-label">API Key</span>
                <BaseInput v-model="apiKey" type="password" :placeholder="account.api_key_masked || 'oc_sk_...'" />
              </div>
              <div class="settings-row">
                <span class="settings-label">接口基址</span>
                <BaseInput v-model="baseUrl" placeholder="https://opencode.ai/console/api" />
              </div>
              <div class="settings-row">
                <span class="settings-label">同步间隔（秒）</span>
                <BaseInput v-model.number="intervalSecs" type="number" spinner :min="2" :max="3600" />
              </div>
              <div class="settings-row">
                <span class="settings-label">登录授权</span>
                <span class="login-inline">
                  <span class="login-state" :class="{ on: login.logged_in }">{{ login.logged_in ? '已授权' : '未授权' }}</span>
                  <BaseButton variant="primary" @click="startLogin">登录 OpenCode</BaseButton>
                  <BaseButton @click="doLogout" :disabled="!login.logged_in">退出</BaseButton>
                </span>
              </div>
            </div>
            <template #hint>
              <small :class="['ff-hint', msgType]">{{ msg }}</small>
            </template>
          </SettingsCard>

          <SettingsCard title="同步" description="首次启动自动全量同步；之后按间隔增量同步">
            <div class="sync-row">
              <BaseButton @click="doSync(false)" :disabled="syncing || !(account.configured || login.logged_in)">立即同步</BaseButton>
              <BaseButton variant="primary" @click="doSync(true)" :disabled="syncing || !(account.configured || login.logged_in)">
                {{ syncing ? '同步中…' : '全量同步' }}
              </BaseButton>
              <BaseButton variant="danger" @click="clear" :disabled="!account.configured">清除 Key</BaseButton>
            </div>
            <div class="status-lines">
              <span>状态：{{ account.configured ? 'API Key 已配置' : '未配置 Key' }}{{ login.logged_in ? ' · 已授权' : '' }}</span>
              <span v-if="sync.local_rows">本地已同步 {{ sync.local_rows }} 条记录</span>
              <span v-if="sync.last_sync_ms">最近同步：{{ fmtAgo(sync.last_sync_ms) }} · {{ fmtClock(sync.last_sync_ms) }}</span>
              <span :title="sync.source_note">新增：{{ sync.last_added || 0 }} 条 · 本地：{{ sync.local_rows || 0 }} 条</span>
              <span v-if="sync.last_error" class="err">上次错误：{{ sync.last_error }}</span>
            </div>
          </SettingsCard>

          <SettingsCard title="当前额度">
            <div v-if="quota" class="quota-lines">
              <span>计划：{{ quota.plan_name }}</span>
              <span>5 小时：{{ fmtUsd(quota.five_hour.used_micro_cents) }} / {{ fmtUsd(quota.five_hour.limit_micro_cents) }}</span>
              <span>本周：{{ fmtUsd(quota.week.used_micro_cents) }} / {{ fmtUsd(quota.week.limit_micro_cents) }}</span>
              <span>本月：{{ fmtUsd(quota.month.used_micro_cents) }} / {{ fmtUsd(quota.month.limit_micro_cents) }}</span>
              <span>到期：{{ fmtDate(quota.ends_at) }}</span>
            </div>
            <div v-else class="quota-lines"><span class="muted">暂无额度数据</span></div>
          </SettingsCard>
        </div>

        <!-- ── Tab：日志 ── -->
        <div v-show="activeTab === 'logs'" class="tab-pane">
          <SettingsCard :title="login.logged_in ? '逐条请求日志' : '云端日志（按天汇总）'"
            :description="login.logged_in ? '来自 /request-logs，时间精确到秒' : 'Service Key 只能拿按天汇总；登录后可看逐条'" auto>
            <template #actions>
              <DdSelect :options="logRangeOptions" v-model="logRange" />
            </template>
            <div class="log-table-wrap">
              <table class="log-table" v-if="login.logged_in">
                <thead>
                  <tr>
                    <th>时间</th><th>模型</th><th class="num">输入</th><th class="num">输出</th>
                    <th class="num">缓存读</th><th class="num">缓存写</th><th class="num">耗时</th><th class="num">状态</th><th class="num">花费</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(r, i) in reqLogs" :key="i">
                    <td>{{ fmtTime(r.started_at_ms) }}</td>
                    <td class="ellipsis" :title="r.model">{{ r.model }}</td>
                    <td class="num">{{ fmtTokens(r.input_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.output_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.cache_read_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.cache_write_tokens, convertUnits) }}</td>
                    <td class="num">{{ r.duration_ms || 0 }}ms</td>
                    <td class="num" :class="{ bad: r.status_code >= 400 }">{{ r.status_code || '-' }}</td>
                    <td class="num">{{ fmtUsd(r.cost_micro_cents) }}</td>
                  </tr>
                  <tr v-if="!reqLogs.length"><td colspan="9" class="empty">暂无逐条记录，请登录后点「全量同步」</td></tr>
                </tbody>
              </table>

              <table class="log-table" v-else>
                <thead>
                  <tr>
                    <th>日期</th><th>用户</th><th>模型</th>
                    <th class="num">请求</th><th class="num">输入</th>
                    <th class="num">输出</th><th class="num">缓存读</th><th class="num">花费</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(r, i) in logs" :key="i">
                    <td>{{ r.day }}</td>
                    <td class="ellipsis" :title="r.user_name || r.user_id">{{ r.user_name || r.user_id }}</td>
                    <td class="ellipsis" :title="r.model">{{ r.model }}</td>
                    <td class="num">{{ r.requests.toLocaleString() }}</td>
                    <td class="num">{{ fmtTokens(r.input_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.output_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.cache_read_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtUsd(r.cost_micro_cents) }}</td>
                  </tr>
                  <tr v-if="!logs.length"><td colspan="8" class="empty">暂无记录，请先同步</td></tr>
                </tbody>
              </table>
            </div>
            <div class="pager" v-if="logTotal > logPageSize">
              <BaseButton @click="changePage(-1)" :disabled="logPage <= 1">上一页</BaseButton>
              <span class="pager-text">{{ logPage }} / {{ Math.ceil(logTotal / logPageSize) }}</span>
              <BaseButton @click="changePage(1)" :disabled="logPage >= Math.ceil(logTotal / logPageSize)">下一页</BaseButton>
            </div>
          </SettingsCard>
        </div>

        <!-- ── Tab：模型 ── -->
        <div v-show="activeTab === 'models'" class="tab-pane">
          <SettingsCard title="速率（RPM）" description="官方未公开 RPM 上限，以下为近 24 小时实测值">
            <div class="rpm-grid">
              <div class="rpm-cell"><span class="rpm-label">当前 RPM</span><span class="rpm-num">{{ rpm.current }}</span></div>
              <div class="rpm-cell"><span class="rpm-label">近 5 分钟</span><span class="rpm-num">{{ rpm.last_5m }}</span></div>
              <div class="rpm-cell"><span class="rpm-label">峰值 RPM</span><span class="rpm-num">{{ rpm.peak }}</span></div>
              <div class="rpm-cell"><span class="rpm-label">平均 RPM</span><span class="rpm-num">{{ rpm.avg }}</span></div>
              <div class="rpm-cell"><span class="rpm-label">429 限流</span><span class="rpm-num" :class="{ bad: rpm.throttled_429 > 0 }">{{ rpm.throttled_429 }}</span></div>
            </div>
          </SettingsCard>

          <SettingsCard title="模型额度" description="月上限来自官方文档；用量来自已同步的逐条日志" auto>
            <div class="log-table-wrap">
              <table class="log-table">
                <thead>
                  <tr>
                    <th>模型</th><th class="num">月上限</th>
                    <th class="num">5 小时</th><th class="num">本周</th><th class="num">本月请求</th>
                    <th class="num">本月已花</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="m in models" :key="m.id">
                    <td class="ellipsis" :title="m.id">{{ m.name }}<span v-if="!m.known" class="muted"> 未知</span></td>
                    <td class="num">{{ m.limit && m.limit.unlimited ? '不限' : (m.limit ? '$' + m.limit.usd : '—') }}</td>
                    <td class="num">{{ reqPair(m.usage.five_hour.requests, m.limit && m.limit.req_5h) }}</td>
                    <td class="num">{{ reqPair(m.usage.week.requests, m.limit && m.limit.req_week) }}</td>
                    <td class="num">{{ reqPair(m.usage.month.requests, m.limit && m.limit.req_month) }}</td>
                    <td class="num">{{ fmtUsd(m.usage.month.cost_micro_cents) }}</td>
                  </tr>
                  <tr v-if="!models.length"><td colspan="6" class="empty">暂无用量记录（登录后同步逐条日志）</td></tr>
                </tbody>
              </table>
            </div>
          </SettingsCard>
        </div>

        <!-- ── Tab：界面与系统 ── -->
        <div v-show="activeTab === 'system'" class="tab-pane">
          <div class="grid-2">
            <SettingsCard title="关于">
              <div class="about-row">
                <img :src="iconUrl" alt="OpenCode Monitor" class="about-icon" />
                <div class="about-info">
                  <span class="about-name">OpenCode Monitor</span>
                  <span class="about-version">v{{ appVersion }}</span>
                </div>
              </div>
              <div class="settings-row">
                <span class="settings-label">检查更新</span>
                <BaseButton variant="primary" @click="checkForUpdate">
                  <span style="margin-right:4px">⟳</span> 检查更新
                </BaseButton>
              </div>
              <template #hint>
                <small :class="['ff-hint', updateMsgType]">{{ updateMsg }}</small>
              </template>
            </SettingsCard>
            <SettingsCard title="词元数量单位转换" description="开启后超过 1000 显示为 K / M / B">
              <div class="toggle-row">
                <BaseToggle v-model="unitToggle" labelOn="已开启" labelOff="已关闭" />
              </div>
            </SettingsCard>
          </div>

          <div class="grid-2">
            <SettingsCard title="关闭按钮行为" description="点击关闭按钮时的默认操作">
              <DdSelect :options="closeActionOptions" v-model="closeAction" />
              <template #hint>
                <small :class="['ff-hint', closeActionMsgType]">{{ closeActionMsg }}</small>
              </template>
            </SettingsCard>
            <SettingsCard title="开机自启动" description="系统启动时自动运行程序">
              <div class="toggle-row">
                <BaseToggle v-model="autostart" labelOn="已开启" labelOff="已关闭" />
              </div>
              <template #hint>
                <small :class="['ff-hint', autostartMsgType]">{{ autostartMsg }}</small>
              </template>
            </SettingsCard>
          </div>

          <SettingsCard title="界面主题">
            <div class="theme-row">
              <div
                v-for="t in themeOptions"
                :key="t.v"
                class="theme-card"
                :class="{ active: themeName === t.v }"
                @click="setTheme(t.v)"
              >
                <span class="theme-icon"><ThemeIcon :name="t.icon" /></span>
                <div class="theme-info">
                  <span class="theme-name">{{ t.label }}</span>
                  <span class="theme-desc">{{ t.desc }}</span>
                </div>
                <svg v-if="themeName === t.v" class="theme-check" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
              </div>
            </div>
          </SettingsCard>
        </div>

        </div><!-- /tab-content -->
        </div><!-- /modal-body -->

        <div class="modal-footer">
          <span class="footer-hint">配置修改立即生效</span>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import IconButton from './base/IconButton.vue'
import BaseButton from './base/BaseButton.vue'
import BaseInput from './base/BaseInput.vue'
import BaseToggle from './base/BaseToggle.vue'
import SettingsCard from './SettingsCard.vue'
import DdSelect from './DdSelect.vue'
import ThemeIcon from './ThemeIcon.vue'
import { useTauri } from '../composables/useTauri'
import { fmtUsd, fmtTokens, fmtDate, fmtAgo, fmtClock } from '../utils/format'
import { version as pkgVersion } from '../../package.json'
import iconUrl from '../../icons/icon.png'

const { invoke } = useTauri()

const props = defineProps({
  visible: { type: Boolean, default: false },
  themeName: { type: String, default: 'dark' },
  convertUnits: { type: Boolean, default: false },
  login: { type: Object, default: () => ({ logged_in: false, org_id: '' }) },
  autoLogin: { type: Boolean, default: false },
})
const emit = defineEmits(['close', 'update:themeName', 'update:convertUnits', 'changed', 'login-changed'])

const appVersion = ref(pkgVersion || '0.0.0')
const activeTab = ref('account')
const tabs = [
  { id: 'account', label: '账户' },
  { id: 'models', label: '模型' },
  { id: 'logs', label: '日志' },
  { id: 'system', label: '界面与系统' },
]

// ── 账户 ──
const account = ref({ configured: false, api_key_masked: '', base_url: '' })
const apiKey = ref('')
const baseUrl = ref('https://opencode.ai/console/api')
const intervalSecs = ref(5)
const syncing = ref(false)
const msg = ref('')
const msgType = ref('')
const quota = ref(null)
const sync = ref({ configured: false, syncing: false, last_sync_ms: 0, last_full_sync_ms: 0, last_error: null, local_rows: 0 })

const _loading = ref(true)

async function loadAccount() {
  try {
    account.value = await invoke('get_account')
    baseUrl.value = account.value.base_url || 'https://opencode.ai/console/api'
    intervalSecs.value = account.value.incremental_secs || 5
  } catch {}
  await loadQuota()
  await loadSync()
}

async function loadQuota() {
  try {
    const r = await invoke('get_quota')
    quota.value = r?.available ? r.quota : null
  } catch { quota.value = null }
}

async function loadSync() {
  try { sync.value = (await invoke('get_sync_status')) || sync.value } catch {}
}

// API Key 自动保存（防抖 900ms），不再需要「保存」按钮
let keyTimer = null
async function autoSaveKey() {
  const val = (apiKey.value || '').trim()
  if (val.length < 8) {
    msg.value = val ? 'API Key 太短' : ''
    return
  }
  msg.value = '保存并验证中…'; msgType.value = ''
  try {
    const r = await invoke('save_account', {
      apiKey: val,
      baseUrl: baseUrl.value,
      incrementalSecs: Number(intervalSecs.value) || 5,
    })
    quota.value = r.quota
    account.value = { ...account.value, configured: true, api_key_masked: '****' }
    msg.value = `✓ 已保存并验证（${r.quota.plan_name}）`; msgType.value = 'ok'
    await loadSync()
    emit('changed')
  } catch (e) {
    msg.value = String(e); msgType.value = 'err'
  }
}

watch(apiKey, () => {
  if (_loading.value) return
  clearTimeout(keyTimer)
  keyTimer = setTimeout(autoSaveKey, 900)
})

// 基址 / 间隔：防抖自动保存（不改动 API Key）
let setTimer = null
function scheduleSettings() {
  if (_loading.value) return
  clearTimeout(setTimer)
  setTimer = setTimeout(async () => {
    try {
      await invoke('save_settings', {
        baseUrl: baseUrl.value,
        incrementalSecs: Number(intervalSecs.value) || 5,
      })
      msg.value = '✓ 已保存'; msgType.value = 'ok'
    } catch (e) { msg.value = String(e); msgType.value = 'err' }
  }, 700)
}
watch(baseUrl, scheduleSettings)
watch(intervalSecs, scheduleSettings)

async function clear() {
  try {
    await invoke('clear_account')
    apiKey.value = ''; quota.value = null
    msg.value = '✓ 已清除 API Key'; msgType.value = 'ok'
    await loadAccount()
    emit('changed')
  } catch (e) { msg.value = String(e); msgType.value = 'err' }
}

async function doSync(full) {
  syncing.value = true
  try {
    const r = props.login?.logged_in
      ? await invoke('sync_request_logs_now', { full })
      : (full ? await invoke('sync_full_now') : await invoke('sync_now'))
    msg.value = `✓ 已处理 ${r.rows} 条`; msgType.value = 'ok'
    await loadSync()
    await loadLogs()
    emit('changed')
  } catch (e) { msg.value = String(e); msgType.value = 'err' }
  finally { syncing.value = false }
}

// ── 登录授权（与 API Key 同一面板，状态统一走 msg）──
let loginPoll = null

async function startLogin() {
  msg.value = '正在打开登录窗口…'; msgType.value = ''
  try {
    await invoke('open_login_window')
    msg.value = '请在弹出窗口完成登录…'
    clearInterval(loginPoll)
    loginPoll = setInterval(async () => {
      try {
        const r = await invoke('capture_login')
        if (r?.ok) {
          clearInterval(loginPoll); loginPoll = null
          msg.value = '✓ 授权成功'; msgType.value = 'ok'
          emit('login-changed')
          emit('changed')
        }
      } catch {}
    }, 1500)
  } catch (e) { msg.value = String(e); msgType.value = 'err' }
}

async function doLogout() {
  clearInterval(loginPoll); loginPoll = null
  try {
    await invoke('logout')
    msg.value = '✓ 已退出登录'; msgType.value = 'ok'
    emit('login-changed')
  } catch (e) { msg.value = String(e); msgType.value = 'err' }
}

watch(() => props.autoLogin, (v) => { if (v && props.visible) startLogin() })

onBeforeUnmount(() => { clearInterval(loginPoll) })

// ── 日志 ──
const logs = ref([])
const reqLogs = ref([])
const models = ref([])
const rpm = ref({ current: 0, peak: 0, avg: 0, last_5m: 0, throttled_429: 0, total: 0 })

function fmtTime(ms) {
  if (!ms) return '--'
  const d = new Date(ms)
  const p = (n) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

function reqPair(actual, est) {
  const a = Number(actual || 0).toLocaleString()
  if (est === undefined || est === null) return a
  if (est < 0) return `${a} / 不限`
  return `${a} / ${Number(est).toLocaleString()}`
}
const logTotal = ref(0)
const logPage = ref(1)
const logPageSize = 50
const logRange = ref('7d')
const logRangeOptions = [
  { v: '7d', label: '近 7 天' },
  { v: '30d', label: '近 30 天' },
  { v: 'all', label: '全部' },
]

async function loadLogs() {
  if (props.login?.logged_in) {
    try {
      const r = await invoke('get_request_logs', {
        range: logRange.value, page: logPage.value, pageSize: logPageSize,
      })
      reqLogs.value = r?.rows || []
      logTotal.value = r?.total || 0
    } catch { reqLogs.value = []; logTotal.value = 0 }
    logs.value = []
  } else {
    try {
      const r = await invoke('get_usage_rows', {
        range: logRange.value, page: logPage.value, pageSize: logPageSize,
      })
      logs.value = r?.rows || []
      logTotal.value = r?.total || 0
    } catch { logs.value = []; logTotal.value = 0 }
    reqLogs.value = []
  }
}

async function loadModels() {
  try { models.value = (await invoke('get_models'))?.models || [] } catch { models.value = [] }
}

async function loadRpm() {
  try { rpm.value = (await invoke('get_rpm', { range: '24h' })) || rpm.value } catch {}
}

function changePage(delta) {
  logPage.value = Math.max(1, logPage.value + delta)
  loadLogs()
}

// ── 系统 ──
const closeAction = ref('ask')
const closeActionMsg = ref('')
const closeActionMsgType = ref('')
const autostart = ref(false)
const autostartMsg = ref('')
const autostartMsgType = ref('')
const updateMsg = ref('')
const updateMsgType = ref('')
const closeActionOptions = [
  { v: 'ask', label: '询问' },
  { v: 'minimize', label: '最小化到托盘' },
  { v: 'quit', label: '关闭程序' },
]
const themeOptions = [
  { v: 'dark', label: '深色', desc: '暗色护眼', icon: 'dark' },
  { v: 'light', label: '浅色', desc: '明亮简洁', icon: 'light' },
  { v: 'system', label: '跟随系统', desc: '自动切换', icon: 'system' },
]
const unitToggle = computed({
  get: () => props.convertUnits,
  set: (v) => emit('update:convertUnits', v),
})
function setTheme(v) { emit('update:themeName', v) }

async function loadSystem() {
  try { closeAction.value = (await invoke('get_close_action')).action || 'ask' } catch {}
  try { autostart.value = await invoke('get_autostart') } catch {}
}

watch(closeAction, async (v) => {
  if (_loading.value) return
  try {
    await invoke('set_close_action', { action: v })
    closeActionMsg.value = '✓ 已保存'; closeActionMsgType.value = 'ok'
  } catch (e) { closeActionMsg.value = String(e); closeActionMsgType.value = 'err' }
})

watch(autostart, async (v) => {
  if (_loading.value) return
  try {
    await invoke('set_autostart', { enabled: v })
    autostartMsg.value = v ? '✓ 已开启开机自启' : '✓ 已关闭开机自启'
    autostartMsgType.value = 'ok'
  } catch (e) { autostartMsg.value = String(e); autostartMsgType.value = 'err' }
})

watch(logRange, () => { logPage.value = 1; loadLogs() })

watch(() => props.visible, async (v) => {
  if (!v) return
  _loading.value = true
  activeTab.value = 'account'
  msg.value = ''
  await loadAccount()
  await loadLogs()
  await loadModels()
  await loadRpm()
  await loadSystem()
  // 略等一拍，避免 loadAccount 赋值的 watcher 在 _loading 变 false 后才触发而误保存
  setTimeout(() => { _loading.value = false }, 300)
})

function switchTab(id) {
  activeTab.value = id
  if (id === 'logs') loadLogs()
  if (id === 'models') { loadModels(); loadRpm() }
}

async function checkForUpdate() {
  updateMsg.value = '检查中…'; updateMsgType.value = ''
  try {
    const resp = await fetch('https://api.github.com/repos/icehomura/opencode-monitor/releases/latest')
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    const data = await resp.json()
    const latestTag = (data.tag_name || '').replace(/^v/, '')
    if (!latestTag) { updateMsg.value = '无法获取最新版本信息'; return }
    if (latestTag === appVersion.value) {
      updateMsg.value = `✓ 当前已是最新版本 v${appVersion.value}`; updateMsgType.value = 'ok'
    } else {
      updateMsg.value = `新版本 v${latestTag} 可用（当前 v${appVersion.value}）`
    }
  } catch (e) { updateMsg.value = `检查失败：${String(e)}`; updateMsgType.value = 'err' }
}
</script>

<style scoped>
.modal-mask {
  position: fixed; inset: 0;
  background: rgba(5, 10, 20, .55);
  backdrop-filter: blur(3px);
  display: flex; align-items: center; justify-content: center;
  z-index: 100;
}
.modal {
  width: 760px; max-width: calc(100vw - 48px);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px 24px 14px;
  box-shadow: 0 12px 40px rgba(0,0,0,.35);
  max-height: calc(100vh - 64px);
  display: flex; flex-direction: column;
}
.modal-body { overflow-y: auto; min-height: 0; margin: 0 -4px; padding: 0 4px; scrollbar-gutter: stable; }
.modal-head { display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: 12px; flex-shrink: 0; }
.modal-head h2 { font-size: 16px; font-weight: 600; margin: 0; }
.modal-sub { font-size: 12px; color: var(--muted); margin: 2px 0 0; }
.modal-close { width: 34px !important; height: 34px !important; flex-shrink: 0; border-radius: 6px; }

.tabs { display: flex; gap: 2px; margin-bottom: 12px; background: var(--bg); border-radius: 8px; padding: 3px; flex-shrink: 0; }
.tab-btn { flex: 1; padding: 7px 12px; font-size: 12px; font-weight: 500; color: var(--muted); background: transparent; border: none; border-radius: 6px; cursor: pointer; transition: all .15s; }
.tab-btn:hover { color: var(--text); }
.tab-btn.active { background: var(--panel); color: var(--text); font-weight: 600; }

.tab-pane > .settings-card, .tab-pane > .grid-2 { margin-bottom: 10px; }
.tab-pane > :last-child { margin-bottom: 0; }
.grid-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: stretch; }

.settings-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; width: 100%; min-width: 0; }
.settings-label { font-size: 12px; color: var(--muted); flex-shrink: 0; }
.settings-row .input-wrap { flex: 0 0 320px; min-width: 0; }
.form-col { display: flex; flex-direction: column; gap: 10px; width: 100%; }
.sync-row { display: flex; gap: 8px; flex-wrap: wrap; }
.login-inline { display: flex; align-items: center; gap: 8px; }
.login-state { font-size: 11px; color: var(--muted); background: var(--border); border-radius: 10px; padding: 2px 8px; }
.login-state.on { color: var(--green); }
.status-lines, .quota-lines { display: flex; flex-direction: column; gap: 4px; margin-top: 10px; font-size: 12px; color: var(--muted); }
.quota-lines .muted { color: var(--muted); }
.status-lines .err { color: #ff6b6b; }

.ff-hint { display: block; margin-top: 8px; margin-left: auto; text-align: right; color: var(--muted); font-size: 11px; min-height: 14px; max-width: 100%; }
.ff-hint.ok { color: var(--green); }
.ff-hint.err { color: #ff6b6b; }

.log-table-wrap { width: 100%; overflow-x: auto; }
.log-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.log-table th { text-align: left; color: var(--muted); font-weight: 500; padding: 6px 8px; border-bottom: 1px solid var(--border); white-space: nowrap; }
.log-table td { padding: 6px 8px; border-bottom: 1px solid var(--border); color: var(--text); white-space: nowrap; }
.log-table .num { text-align: right; }
.log-table .ellipsis { max-width: 180px; overflow: hidden; text-overflow: ellipsis; }
.log-table .empty { text-align: center; color: var(--muted); padding: 18px; }
.log-table .bad { color: #ff6b6b; }
.log-table .muted { color: var(--muted); font-size: 10px; }
.rpm-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 10px; width: 100%; }
.rpm-cell { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 8px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; }
.rpm-label { font-size: 11px; color: var(--muted); }
.rpm-num { font-size: 18px; font-weight: 700; color: var(--text); }
.rpm-num.bad { color: #ff6b6b; }
.pager { display: flex; align-items: center; justify-content: center; gap: 12px; margin-top: 10px; }
.pager-text { font-size: 12px; color: var(--muted); }

.toggle-row { display: flex; align-items: center; justify-content: space-between; }

.about-row { display: flex; align-items: center; gap: 12px; margin-bottom: 10px; }
.about-icon { width: 48px; height: 48px; border-radius: 10px; }
.about-info { display: flex; flex-direction: column; gap: 2px; }
.about-name { font-size: 15px; font-weight: 600; color: var(--text); }
.about-version { font-size: 12px; color: var(--muted); }

.theme-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.theme-card { display: flex; align-items: center; gap: 10px; padding: 12px 14px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; cursor: pointer; transition: border-color .15s, background .15s; position: relative; }
.theme-card:hover { border-color: var(--muted); }
.theme-card.active { border-color: var(--blue); background: rgba(79,140,255,.08); }
.theme-icon { font-size: 22px; flex-shrink: 0; }
.theme-info { display: flex; flex-direction: column; gap: 1px; }
.theme-name { font-size: 13px; font-weight: 600; color: var(--text); }
.theme-desc { font-size: 11px; color: var(--muted); }
.theme-check { position: absolute; top: 8px; right: 8px; color: var(--blue); }

.modal-footer { display: flex; align-items: center; justify-content: flex-end; gap: 16px; padding-top: 14px; margin-top: 12px; border-top: 1px solid var(--border); flex-shrink: 0; }
.footer-hint { font-size: 12px; color: var(--muted); }
</style>
