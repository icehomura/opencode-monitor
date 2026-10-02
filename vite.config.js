import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  // Tauri expects a fixed port
  server: { strictPort: true },
  // 路径别名
  resolve: {
    alias: { '@': '/src' },
  },
})
