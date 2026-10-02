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
          <SettingsCard title="账号管理"
            description="所有账号都会同步日志；标题栏与额度 / 模型卡片只显示主账号" auto>
            <template #actions>
              <div class="acct-head-actions">
                <BaseButton v-if="showPrimaryLogin" @click="loginPrimary">登录主账号</BaseButton>
                <BaseButton variant="primary" @click="addAccount">
                  <span style="margin-right:4px">+</span> 添加账号
                </BaseButton>
              </div>
            </template>

            <label class="acct-ack" :class="{ warn: ackWarn }">
              <input type="checkbox" v-model="ownAccountAck" />
              <span>添加的账号需为<strong>你本人拥有或已获授权访问</strong>；本工具只读展示用量，不做绕过或自动切号。</span>
            </label>

            <div v-if="accounts.length" class="acct-list">
              <div v-for="a in accounts" :key="a.id" class="acct-item">
                <div class="acct-info">
                  <span class="acct-name">
                    <input v-if="renamingId === a.id" class="acct-rename" v-model="renameDraft"
                      @keydown.enter.prevent="commitRename" @keydown.esc.prevent="cancelRename"
                      @blur="commitRename" />
                    <span v-else class="acct-title" title="双击可重命名" @dblclick="startRename(a)">{{ a.name }}</span>
                    <span v-if="a.is_primary" class="acct-badge">主账号</span>
                    <span class="acct-state" :class="{ on: a.logged_in }">{{ a.logged_in ? '已登录' : '未登录' }}</span>
                  </span>
                  <span class="acct-detail">
                    本地 {{ a.rows || 0 }} 条 · 最近同步 {{ fmtAgo(a.last_sync_ms) }} ·
                    <template v-if="a.quota">额度 5 小时 {{ fmtUsd(a.quota.five_hour.used_micro_cents) }} / 周 {{ fmtUsd(a.quota.week.used_micro_cents) }} / 月 {{ fmtUsd(a.quota.month.used_micro_cents) }}</template>
                    <template v-else>额度 —</template>
                  </span>
                  <span v-if="a.org_id" class="acct-org" :title="a.org_id">{{ a.org_id }}</span>
                </div>
                <div class="acct-actions">
                  <BaseButton v-if="!a.is_primary" @click="setPrimaryAccount(a)">设为主账号</BaseButton>
                  <BaseButton @click="reloginAccount(a)">重新登录</BaseButton>
                  <BaseButton @click="logoutAccount(a)" :disabled="!a.logged_in">退出登录</BaseButton>
                  <BaseButton variant="danger" @click="removeAccount(a)">删除</BaseButton>
                </div>
              </div>
            </div>
            <div v-else class="acct-empty">
              <span class="muted">暂无账号，点击右上角「登录主账号」或「添加账号」在弹出窗口登录后自动创建。</span>
            </div>

            <template #hint>
              <small :class="['ff-hint', acctMsgType]">{{ acctMsg }}</small>
            </template>
          </SettingsCard>

          <SettingsCard title="接口与同步间隔" description="接口基址与同步间隔修改后自动保存">
            <div class="form-col">
              <div class="settings-row">
                <span class="settings-label">接口基址</span>
                <BaseInput v-model="baseUrl" placeholder="https://opencode.ai/console/api" />
              </div>
              <div class="settings-row">
                <span class="settings-label">同步间隔（秒）</span>
                <BaseInput v-model.number="intervalSecs" type="number" spinner :min="10" :max="3600" />
              </div>
            </div>
            <template #hint>
              <small :class="['ff-hint', msgType]">{{ msg }}</small>
            </template>
          </SettingsCard>

          <SettingsCard title="同步" description="首次启动自动全量同步；之后按间隔增量同步（一次同步所有已登录账号）">
            <div class="sync-row">
              <BaseButton @click="doSync(false)" :disabled="syncing || !anyLoggedIn">立即同步</BaseButton>
              <BaseButton variant="primary" @click="doSync(true)" :disabled="syncing || !anyLoggedIn">
                {{ syncing ? '同步中…' : '全量同步' }}
              </BaseButton>
            </div>
            <div class="status-lines">
              <span>状态：{{ anyLoggedIn ? `已登录 ${loggedInCount} / ${accounts.length} 个账号` : '未登录' }}</span>
              <span v-if="sync.local_rows">本地已同步 {{ sync.local_rows }} 条记录</span>
              <span v-if="sync.last_sync_ms">最近同步：{{ fmtAgo(sync.last_sync_ms) }} · {{ fmtClock(sync.last_sync_ms) }}</span>
              <span :title="sync.source_note">新增：{{ sync.last_added || 0 }} 条 · 本地：{{ sync.local_rows || 0 }} 条</span>
              <span v-if="sync.last_error" class="err">上次错误：{{ sync.last_error }}</span>
            </div>
          </SettingsCard>

          <SettingsCard title="当前额度（主账号）" description="来自主账号的控制台额度，随账号与同步更新">
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
          <SettingsCard title="逐条请求日志"
            description="来自控制台 /request-logs，时间精确到秒；可按账号筛选，未登录时无数据" auto>
            <template #actions>
              <div class="logs-actions">
                <DdSelect :options="logAccountOptions" v-model="logAccount" />
                <DdSelect :options="logRangeOptions" v-model="logRange" />
              </div>
            </template>
            <div class="log-table-wrap">
              <table class="log-table">
                <thead>
                  <tr>
                    <th>时间</th><th>账号</th><th>模型</th><th class="num">输入</th><th class="num">输出</th>
                    <th class="num">缓存读</th><th class="num">缓存写</th><th class="num">耗时</th><th class="num">状态</th><th class="num">花费</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(r, i) in reqLogs" :key="i">
                    <td>{{ fmtTime(r.started_at_ms) }}</td>
                    <td class="ellipsis" :title="accountName(r.account_id)">{{ accountName(r.account_id) }}</td>
                    <td class="ellipsis" :title="r.model">{{ r.model }}</td>
                    <td class="num">{{ fmtTokens(r.input_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.output_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.cache_read_tokens, convertUnits) }}</td>
                    <td class="num">{{ fmtTokens(r.cache_write_tokens, convertUnits) }}</td>
                    <td class="num">{{ r.duration_ms || 0 }}ms</td>
                    <td class="num" :class="{ bad: r.status_code >= 400 }">{{ r.status_code || '-' }}</td>
                    <td class="num">{{ fmtUsd(r.cost_micro_cents) }}</td>
                  </tr>
                  <tr v-if="!reqLogs.length"><td colspan="10" class="empty">{{ logEmptyText }}</td></tr>
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
          <span class="footer-decl">只读同步工具：不绕过任何额度 / 限流 / 计费限制，不自动切换账号，不采集提示词与模型输出，不上传数据</span>
          <span class="footer-hint">配置修改立即生效</span>
        </div>
      </div>
    </div>

  </Teleport>
</template>

<script setup>
import { ref, watch, computed, nextTick, onBeforeUnmount } from 'vue'
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
const baseUrl = ref('https://opencode.ai/console/api')
const intervalSecs = ref(30)
// 「添加账号」前的一次性确认（只添加自己拥有 / 已获授权的账号）
const ownAccountAck = ref(localStorage.getItem('ocm_own_account_ack') === '1')
const ackWarn = ref(false)
let ackTimer = null
watch(ownAccountAck, (v) => {
  localStorage.setItem('ocm_own_account_ack', v ? '1' : '0')
  if (v) ackWarn.value = false
})
const syncing = ref(false)
const msg = ref('')
const msgType = ref('')
const quota = ref(null)
const sync = ref({ syncing: false, last_sync_ms: 0, last_full_sync_ms: 0, last_error: null, local_rows: 0 })

// 账号管理：列表来自 list_accounts（含每账号额度 / 本地条数 / 同步水位）
const accounts = ref([])
const acctMsg = ref('')
const acctMsgType = ref('')
const renamingId = ref('')
const renameDraft = ref('')
const primaryAccount = computed(() => accounts.value.find((a) => a.is_primary) || null)
const anyLoggedIn = computed(() => accounts.value.some((a) => a.logged_in))
const loggedInCount = computed(() => accounts.value.filter((a) => a.logged_in).length)
const showPrimaryLogin = computed(() => !primaryAccount.value || !primaryAccount.value.logged_in)

const _loading = ref(true)

async function loadSettings() {
  try {
    const s = await invoke('get_settings')
    baseUrl.value = s?.base_url || 'https://opencode.ai/console/api'
    intervalSecs.value = s?.incremental_secs || 30
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

async function loadAccounts() {
  try {
    const r = await invoke('list_accounts')
    accounts.value = r?.accounts || []
  } catch { accounts.value = [] }
}

function accountName(id) {
  if (!id) return '—'
  return accounts.value.find((a) => a.id === id)?.name || id
}

// 主账号未登录 / 无账号时的登录入口
function loginPrimary() { startLogin() }
// 添加新账号：登录结果会新建一个账号。未勾选确认时不禁用按钮，而是明确提示（避免"灰按钮无解释"）
function addAccount() {
  if (!ownAccountAck.value) {
    ackWarn.value = true
    acctMsg.value = '添加账号前，请先勾选上方「账号需为本人所有或已获授权」'
    acctMsgType.value = 'err'
    clearTimeout(ackTimer)
    ackTimer = setTimeout(() => { ackWarn.value = false }, 2600)
    return
  }
  acctMsg.value = ''
  acctMsgType.value = ''
  startLogin({ addNew: true })
}
function reloginAccount(a) { startLogin({ accountId: a.id }) }

// 重命名：双击账号名就地编辑（不再有「编辑」按钮 / 编辑弹窗）
async function startRename(a) {
  renamingId.value = a.id
  renameDraft.value = a.name
  await nextTick()
  const el = document.querySelector('.acct-rename')
  el?.focus()
  el?.select?.()
}
function cancelRename() {
  renamingId.value = ''
  renameDraft.value = ''
}
async function commitRename() {
  const id = renamingId.value
  if (!id) return
  const name = renameDraft.value.trim()
  const cur = accounts.value.find((a) => a.id === id)
  cancelRename()
  if (!cur || !name || name === cur.name) return
  try {
    await invoke('rename_account', { id, name })
    acctMsg.value = `✓ 已重命名为「${name}」`; acctMsgType.value = 'ok'
    await loadAccounts()
  } catch (e) { acctMsg.value = String(e); acctMsgType.value = 'err' }
}

async function setPrimaryAccount(a) {
  try {
    await invoke('set_primary_account', { id: a.id })
    acctMsg.value = `✓ 「${a.name}」已设为主账号`; acctMsgType.value = 'ok'
    await loadAccounts()
    await loadQuota()
    emit('changed')
  } catch (e) { acctMsg.value = String(e); acctMsgType.value = 'err' }
}

async function logoutAccount(a) {
  clearInterval(loginPoll); loginPoll = null
  try {
    await invoke('logout', { accountId: a.id })
    acctMsg.value = `✓ 已退出「${a.name}」`; acctMsgType.value = 'ok'
    await loadAccounts()
    await loadQuota()
    emit('login-changed')
    emit('changed')
  } catch (e) { acctMsg.value = String(e); acctMsgType.value = 'err' }
}

// 删除前二次确认，并单独询问是否同时清空本地已同步数据
async function removeAccount(a) {
  if (!window.confirm(`确定删除账号「${a.name}」？此操作不可撤销。`)) return
  const purge = window.confirm(
    `是否同时清空「${a.name}」的本地已同步数据（${a.rows || 0} 条）？\n\n` +
    `「确定」= 删除账号，并清空本地数据\n「取消」= 仅删除账号，保留本地数据`,
  )
  try {
    const r = await invoke('remove_account', { id: a.id, purge })
    acctMsg.value = purge
      ? `✓ 已删除「${a.name}」，并清空 ${r?.purged || 0} 条本地数据`
      : `✓ 已删除「${a.name}」（已保留本地数据）`
    acctMsgType.value = 'ok'
    await loadAccounts()
    if (logAccount.value && !accounts.value.some((a) => a.id === logAccount.value)) logAccount.value = ''
    await loadQuota()
    await loadLogs()
    emit('login-changed')
    emit('changed')
  } catch (e) { acctMsg.value = String(e); acctMsgType.value = 'err' }
}

// 基址 / 间隔：防抖自动保存
let setTimer = null
function scheduleSettings() {
  if (_loading.value) return
  clearTimeout(setTimer)
  setTimer = setTimeout(async () => {
    try {
      await invoke('save_settings', {
        baseUrl: baseUrl.value,
        incrementalSecs: Number(intervalSecs.value) || 30,
      })
      msg.value = '✓ 已保存'; msgType.value = 'ok'
    } catch (e) { msg.value = String(e); msgType.value = 'err' }
  }, 700)
}
watch(baseUrl, scheduleSettings)
watch(intervalSecs, scheduleSettings)

async function doSync(full) {
  syncing.value = true
  try {
    const r = await invoke('sync_request_logs_now', { full })
    const accN = Array.isArray(r?.accounts) ? r.accounts.length : 0
    const errN = Array.isArray(r?.errors) ? r.errors.length : 0
    msg.value = `✓ 已处理 ${r?.rows || 0} 条${accN ? ` · ${accN} 个账号` : ''}${errN ? ` · ${errN} 个账号失败` : ''}`
    msgType.value = errN ? 'err' : 'ok'
    await loadSync()
    await loadAccounts()
    await loadLogs()
    emit('changed')
  } catch (e) { msg.value = String(e); msgType.value = 'err' }
  finally { syncing.value = false }
}

// ── 登录授权（状态统一走 acctMsg）──
let loginPoll = null

// opts.addNew = 添加新账号；opts.accountId = 重新登录指定账号；均空 = 主账号
async function startLogin(opts = {}) {
  const args = {}
  if (opts.addNew) args.addNew = true
  if (opts.accountId) args.accountId = opts.accountId
  acctMsg.value = opts.addNew ? '正在打开登录窗口（添加账号）…' : '正在打开登录窗口…'
  acctMsgType.value = ''
  try {
    await invoke('open_login_window', args)
    acctMsg.value = '请在弹出窗口完成登录…'
    clearInterval(loginPoll)
    loginPoll = setInterval(async () => {
      try {
        const r = await invoke('capture_login')
        if (r?.ok) {
          clearInterval(loginPoll); loginPoll = null
          acctMsg.value = '✓ 授权成功'; acctMsgType.value = 'ok'
          await loadAccounts()
          await loadQuota()
          emit('login-changed')
          emit('changed')
        }
      } catch {}
    }, 1500)
  } catch (e) { acctMsg.value = String(e); acctMsgType.value = 'err' }
}

watch(() => props.autoLogin, (v) => { if (v && props.visible) startLogin() })

onBeforeUnmount(() => { clearInterval(loginPoll) })

// ── 日志 ──
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
// 账号筛选：空 = 全部账号
const logAccount = ref('')
const logAccountOptions = computed(() => [
  { v: '', label: '全部账号' },
  ...accounts.value.map((a) => ({ v: a.id, label: a.name })),
])
const logEmptyText = computed(() => {
  if (!accounts.value.length) return '暂无账号，请到「账户」页登录或添加账号'
  if (!anyLoggedIn.value) return '未登录，请到「账户」页登录'
  return '暂无记录，请到「账户」页点「全量同步」'
})

async function loadLogs() {
  if (!anyLoggedIn.value) { reqLogs.value = []; logTotal.value = 0; return }
  try {
    const r = await invoke('get_request_logs', {
      range: logRange.value, accountId: logAccount.value, page: logPage.value, pageSize: logPageSize,
    })
    reqLogs.value = r?.rows || []
    logTotal.value = r?.total || 0
  } catch { reqLogs.value = []; logTotal.value = 0 }
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
watch(logAccount, () => { logPage.value = 1; loadLogs() })

watch(() => props.visible, async (v) => {
  if (!v) { cancelRename(); return }
  _loading.value = true
  activeTab.value = 'account'
  msg.value = ''
  acctMsg.value = ''
  await loadSettings()
  await loadAccounts()
  await loadLogs()
  await loadModels()
  await loadRpm()
  await loadSystem()
  // 略等一拍，避免 loadSettings 赋值的 watcher 在 _loading 变 false 后才触发而误保存
  setTimeout(() => { _loading.value = false }, 300)
})

function switchTab(id) {
  activeTab.value = id
  if (id === 'account') loadAccounts()
  if (id === 'logs') { loadAccounts().then(loadLogs) }
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
.acct-head-actions { display: flex; align-items: center; gap: 8px; }
.acct-ack {
  display: flex; align-items: flex-start; gap: 8px; margin: 0 0 10px;
  padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px;
  background: rgba(255, 255, 255, .02);
  font-size: 11px; line-height: 1.5; color: var(--muted); cursor: pointer;
  transition: border-color .15s, background .15s;
}
.acct-ack.warn { border-color: #e0a83c; background: rgba(224, 168, 60, .10); }
.acct-ack input { margin-top: 2px; flex-shrink: 0; }
.acct-ack strong { color: var(--text); font-weight: 600; }
.acct-list { display: flex; flex-direction: column; gap: 8px; width: 100%; }
.acct-item {
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
  padding: 10px 12px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px;
}
.acct-info { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.acct-name { display: flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; color: var(--text); min-width: 0; }
.acct-title { max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: text; }
.acct-rename {
  width: 200px; max-width: 240px; font: inherit; color: var(--text);
  background: var(--bg); border: 1px solid var(--blue); border-radius: 6px;
  padding: 1px 6px; outline: none;
}
.acct-badge {
  font-size: 10px; font-weight: 500; color: var(--blue); white-space: nowrap;
  background: rgba(79,140,255,.12); border: 1px solid rgba(79,140,255,.30);
  border-radius: 8px; padding: 0 6px; line-height: 1.6;
}
.acct-state { font-size: 10px; font-weight: 500; color: var(--muted); background: var(--border); border-radius: 8px; padding: 0 6px; line-height: 1.6; white-space: nowrap; }
.acct-state.on { color: var(--green); }
.acct-detail { font-size: 11px; color: var(--muted); }
.acct-org { font-size: 10px; color: var(--muted); opacity: .8; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.acct-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; flex-shrink: 0; max-width: 60%; }
.acct-actions .btn { padding: 4px 10px; font-size: 12px; }
.acct-empty { padding: 14px 0; text-align: center; }
.acct-empty .muted { font-size: 12px; color: var(--muted); }
.logs-actions { display: flex; align-items: center; gap: 8px; }
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
.footer-decl { margin-right: auto; font-size: 11px; color: var(--muted); opacity: .85; }
.footer-hint { font-size: 12px; color: var(--muted); }
</style>
