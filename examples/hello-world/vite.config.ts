import { defineConfig } from 'vite'
import bolt from './vite-plugin-bolt.ts'

export default defineConfig({
  plugins: [bolt()],
})
