import { ref, computed } from 'vue'

const STORAGE_KEY = 'tm_theme'
const mediaDark = window.matchMedia('(prefers-color-scheme: dark)')

const themeName = ref(localStorage.getItem(STORAGE_KEY) || 'dark')

export const themeColors = computed(() =>
  themeName.value === 'light'
    ? { axis: '#d7dee9', label: '#66738c', split: '#e8edf4', blue: '#2f6fe4', green: '#149e78' }
    : { axis: '#263049', label: '#8b97b0', split: '#1c2436', blue: '#4f8cff', green: '#35d0a5' }
)

export function isLight() {
  return themeName.value === 'light'
}

export function hexToRgba(hex, alpha) {
  const n = parseInt(hex.slice(1), 16)
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`
}

function applyThemeAttr() {
  document.documentElement.dataset.theme = currentTheme()
}

function currentTheme() {
  const pref = themeName.value
  return pref === 'system' ? (mediaDark.matches ? 'dark' : 'light') : pref
}

export function useTheme() {
  function setTheme(v) {
    themeName.value = v
    localStorage.setItem(STORAGE_KEY, v)
    applyThemeAttr()
  }

  // system 主题变化时自动切换
  mediaDark.addEventListener('change', () => {
    if (themeName.value === 'system') applyThemeAttr()
  })

  applyThemeAttr()

  return { themeName, setTheme, themeColors, isLight }
}
