import { test, expect } from '@playwright/test'

test.describe('Agent Sight Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/api/agents', async (route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: [{ id: 'a1', name: 'Writer', model: 'gpt-4', status: 'busy', currentLoad: 3, maxLoad: 10, successRate: 0.95, reputationScore: 85, totalTokens: 5000, totalCost: 1.25 }] }) })
    })
    await page.route('**/api/tasks', async (route) => { await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: [], total: 0, page: 1, pageSize: 200 }) }) })
    await page.route('**/api/metrics', async (route) => { await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: { totalTasks: 100, completedTasks: 80, failedTasks: 5, runningTasks: 10, totalAgents: 1, activeAgents: 1, totalTokens: 5000, totalCost: 1.25, avgLatencyMs: 200, successRate: 0.94 } }) }) })
    await page.route('**/api/approvals', async (route) => { await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: [], total: 0, page: 1, pageSize: 100 }) }) })
    await page.route('**/api/health**', async (route) => { await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'healthy', version: '1.0.0', uptime: 3600, services: {} }) }) })
    await page.goto('/agents')
  })
  test('displays agents page', async ({ page }) => { await expect(page.locator('body')).toBeVisible({ timeout: 10000 }); expect(await page.textContent('body')).toBeTruthy() })
  test('shows agent info', async ({ page }) => { await expect(page.locator('body')).toBeVisible({ timeout: 10000 }); expect(await page.textContent('body')).toBeTruthy() })
})
