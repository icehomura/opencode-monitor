const UNIT_KEY = 'tm_unit_convert'

export function getConvertUnits() {
  return localStorage.getItem(UNIT_KEY) === '1'
}

export function setConvertUnits(v) {
  localStorage.setItem(UNIT_KEY, v ? '1' : '0')
}

export function fmtTokens(n, convertUnits) {
  if (!convertUnits) return (n || 0).toLocaleString()
  if (n >= 1e9) return (n / 1e9).toFixed(1) + 'B'
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M'
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K'
  return String(n || 0)
}


/** 剩余占比（0~100），limit 为 0 时返回 0 */
export function remainPct(used, limit) {
  if (!limit || limit <= 0) return 0
  return Math.max(0, Math.min(100, (1 - used / limit) * 100))
}

/** ISO 时间 -> 本地可读；空值返回 '--' */
export function fmtDateTime(iso) {
  if (!iso) return '--'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return '--'
  const p = (n) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}

export function fmtDate(iso) {
  if (!iso) return '--'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return '--'
  const p = (n) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

/** 相对时间：刚刚 / N 分钟前 / N 小时前 */
export function fmtAgo(ms) {
  if (!ms) return '尚未同步'
  const diff = Date.now() - ms
  if (diff < 15_000) return '刚刚'
  if (diff < 60_000) return `${Math.floor(diff / 1000)} 秒前`
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`
  return `${Math.floor(diff / 86_400_000)} 天前`
}

/** 绝对时间 HH:MM:SS（本地时区） */
export function fmtClock(ms) {
  if (!ms) return '--'
  const d = new Date(ms)
  const p = (n) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

function currencySymbol(code) {
  switch (String(code || '').toUpperCase()) {
    case 'USD': return '$'
    case 'CNY': return '¥'
    default: return ''
  }
}

export function fmtBalance(currency, total) {
  const amount = (total === null || total === undefined || total === '') ? '--' : String(total)
  const sym = currencySymbol(currency)
  if (sym) return `${sym}${amount}`
  return currency ? `${amount} ${String(currency).toUpperCase()}` : amount
}
