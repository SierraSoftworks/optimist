import { defineConfig, devices } from '@playwright/test'

/**
 * End-to-end tests against a real server.
 *
 * There is no mock API. The workbench's job is to be right about what the solver
 * says, and a mock would be a second implementation of the server's behaviour
 * maintained by hand — which is exactly the thing most likely to be wrong in the
 * same direction as the code under test.
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: 'http://127.0.0.1:5174',
    trace: 'retain-on-failure',
    // The markup uses `data-test`; the default would look for `data-testid`.
    testIdAttribute: 'data-test',
  },
  projects: [{ name: 'desktop', use: { ...devices['Desktop Chrome'] } }],
  globalSetup: './e2e/support/seed.ts',
  webServer: [
    {
      // Run from the repository root so cargo finds its manifest, which means
      // the designs path has to be written from there too.
      command:
        'cargo run --quiet -- serve --designs ./workbench/e2e/.designs --bind 127.0.0.1:3210',
      cwd: '..',
      url: 'http://127.0.0.1:3210/api/v1/health',
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
    {
      command: 'node_modules/.bin/vite --host 127.0.0.1 --port 5174 --strictPort',
      env: { OPTIMIST_API_URL: 'http://127.0.0.1:3210' },
      url: 'http://127.0.0.1:5174',
      reuseExistingServer: !process.env.CI,
      timeout: 60_000,
    },
  ],
})
