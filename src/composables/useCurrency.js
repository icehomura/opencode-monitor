import { reactive } from 'vue'

/**
 * 汇率换算（显示层）：把界面上的美元金额按 `USD→CNY 汇率 / 换算系数` 显示成人民币。
 *
 * - 开关一：美元 → 人民币（× 汇率）；开关二：再 ÷ 计划价值比（默认 6，$10 计划得 $60 额度 ≈ 1:6），
 *   两个都开就是「真实花销」= 美元 × 汇率 ÷ 6。
 * - 只影响显示，不改动任何数据；关闭开关即回到美元。
 * - 本地化：美元符号在前（`$1.23`），人民币符号在后（`1.23¥`），都保留两位小数。
 */
const KEY = 'ocm_currency'
const saved = (() => {
  try {
    return JSON.parse(localStorage.getItem(KEY) || '{}') || {}
  } catch {
    return {}
  }
})()

export const currency = reactive({
  /** 开关一：是否把美元换算成人民币 */
  enabled: saved.enabled === true,
  /** 开关二：是否再除以计划价值比（真实花销） */
  valueRatio: saved.valueRatio !== false,
  /** USD → CNY */
  rate: Number(saved.rate) || 0,
  /** 计划价值比（默认 6） */
  divisor: Number(saved.divisor) || 6,
  source: saved.source || '',
  at: Number(saved.at) || 0,
  loading: false,
  error: '',
})

export function persistCurrency() {
  localStorage.setItem(KEY, JSON.stringify({
    enabled: currency.enabled,
    valueRatio: currency.valueRatio,
    rate: currency.rate,
    divisor: currency.divisor,
    source: currency.source,
    at: currency.at,
  }))
}

/** 是否处于人民币显示模式。 */
export function currencyOn() {
  return currency.enabled && currency.rate > 0
}

/** 换算后的数值：开启汇率转换时先换算成人民币，再（可选）除以计划价值比。 */
export function convert(usd) {
  const v = Number(usd || 0)
  if (!currencyOn()) return v
  const cny = v * currency.rate
  return currency.valueRatio ? cny / (currency.divisor || 6) : cny
}

/** microCents 金额 → 显示字符串（两位小数）。 */
export function money(microCents) {
  return fmtMoney(Number(microCents || 0) / 1e8, false)
}

/** 原始美元数字 → 显示字符串（用于本来就是美元的字段，如模型上限）。 */
export function moneyUsd(usd) {
  return fmtMoney(Number(usd || 0), false)
}

/** 极小值也尽量显示全（用于「平均金额/分」这类很小的数）。 */
export function moneyFine(microCents) {
  return fmtMoney(Number(microCents || 0) / 1e8, true)
}

function fmtMoney(usd, fine) {
  const on = currencyOn()
  const v = convert(usd)
  const prefix = on ? '' : '$'
  const suffix = on ? '¥' : ''
  if (!fine) return prefix + v.toFixed(2) + suffix
  const abs = Math.abs(v)
  if (abs >= 0.01 || abs === 0) return prefix + v.toFixed(2) + suffix
  if (abs < 1e-8) return (v > 0 ? '<' : '>-') + prefix + '0.00000001' + suffix
  return prefix + String(Number(v.toPrecision(3))) + suffix
}
