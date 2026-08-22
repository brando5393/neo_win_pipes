import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// base is "/" because this is served from the custom domain's root
// (neowinpipes.com/public/CNAME), not the github.io/neo_win_pipes/ project-page
// path — go back to '/neo_win_pipes/' if the custom domain is ever removed.
export default defineConfig({
  base: '/',
  plugins: [react(), tailwindcss()],
})
