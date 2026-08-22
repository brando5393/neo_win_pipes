import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// base must match the GitHub Pages project-page path (https://brando5393.github.io/neo_win_pipes/)
// — a plain "/" here would 404 every asset once deployed, since this isn't served from the domain root.
export default defineConfig({
  base: '/neo_win_pipes/',
  plugins: [react(), tailwindcss()],
})
