import { reactive } from 'vue'

/**
 * 汇率换算（显示层）：把界面上的美元金额按 `USD→CNY 汇率 / 换算系数` 显示成人民币。
 *
 * - 换算系数默认 6：买 $10 的计划得 $60 额度、$40 得 $240，价值比约 1:6，
 *   所以「真实花销」= 美元 × 汇率 ÷ 6。
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
  enabled: saved.enabled === true,
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

/** 换算后的数值（人民币或美元）。 */
export function convert(usd) {
  return currencyOn() ? (Number(usd || 0) * currency.rate) / (currency.divisor || 6) : Number(usd || 0)
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
  const v = on ? (usd * currency.rate) / (currency.divisor || 6) : usd
  const prefix = on ? '' : '$'
  const suffix = on ? '¥' : ''
  if (!fine) return prefix + v.toFixed(2) + suffix
  const abs = Math.abs(v)
  if (abs >= 0.01 || abs === 0) return prefix + v.toFixed(2) + suffix
  if (abs < 1e-8) return (v > 0 ? '<' : '>-') + prefix + '0.00000001' + suffix
  return prefix + String(Number(v.toPrecision(3))) + suffix
}
