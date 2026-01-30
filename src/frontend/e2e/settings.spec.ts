import { test, expect } from '@playwright/test'

test.describe('Settings Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/api/settings', async (route) => {
      if (route.request().method() === 'GET') {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: { maxConcurrentTasks: 50, defaultAgentModel: 'gpt-4' } }) })
      }
    })
    await page.route('**/api/health', async (route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'healthy', version: '1.0.0', uptime: 3600, services: {} }) })
    })
    await page.goto('/settings')
  })
  test('displays settings page', async ({ page }) => {
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })
  })
  test('shows page content', async ({ page }) => {
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })
    expect(await page.textContent('body')).toBeTruthy()
  })
})
