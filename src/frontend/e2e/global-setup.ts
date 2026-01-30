import { FullConfig } from '@playwright/test'

/**
 * Global setup for Playwright E2E tests.
 * Runs once before all test files.
 */
async function globalSetup(config: FullConfig): Promise<void> {
  console.log('Running Playwright global setup...')

  // You can add global setup logic here, such as:
  // - Database seeding
  // - Authentication setup
  // - Environment validation

  // Validate environment
  const baseURL = config.projects[0]?.use?.baseURL || process.env.PLAYWRIGHT_BASE_URL
  if (!baseURL) {
    console.warn('Warning: No baseURL configured. Using default http://localhost:5173')
  }

  console.log(`Test base URL: ${baseURL || 'http://localhost:5173'}`)
  console.log('Global setup complete.')
}

export default globalSetup
