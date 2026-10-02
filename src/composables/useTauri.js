/**
 * Tauri API wrapper — 统一访问 invoke / listen / getCurrentWindow
 * 依赖 tauri.conf.json 中 withGlobalTauri: true
 */
const tauri = window.__TAURI__ || {}

export function useTauri() {
  const invoke = tauri.core?.invoke || (() => { throw new Error('Tauri invoke not available') })
  const listen = tauri.event?.listen || (() => () => {})
  const getCurrentWindow = tauri.window?.getCurrentWindow || (() => null)

  return { invoke, listen, getCurrentWindow }
}
