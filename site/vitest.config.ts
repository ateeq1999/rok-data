import { defineConfig } from 'vitest/config'

// The tests read files; they need none of vite.config.ts's plugins (the
// devtools and nitro plugins keep a test run's server from exiting).
export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
  },
})
