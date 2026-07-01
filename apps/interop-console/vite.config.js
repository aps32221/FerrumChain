import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Ferrum Chapter 9 — cross-border interop operations console.
export default defineConfig({
  plugins: [react()],
  server: { port: 5190 },
})
