import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Ferrum Chapter 11 — federation governance & token operations console.
export default defineConfig({
  plugins: [react()],
  server: { port: 5191 },
})
