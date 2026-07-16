import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests-real',
  reporter: 'line',
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:5174',
    trace: 'retain-on-failure',
    ...devices['Desktop Chrome'],
  },
  webServer: [
    {
      command: 'cargo run --manifest-path ../Cargo.toml -- server --bind 127.0.0.1:3100 --data-dir "$(mktemp -d ../target/workbench-e2e.XXXXXX)"',
      url: 'http://127.0.0.1:3100/api/v1/health',
      reuseExistingServer: false,
    },
    {
      command: 'OPTIMIST_API_URL=http://127.0.0.1:3100 npm run dev -- --port 5174',
      url: 'http://127.0.0.1:5174',
      reuseExistingServer: false,
    },
  ],
})