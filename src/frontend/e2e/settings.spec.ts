import { test, expect } from '@playwright/test'

// ═══════════════════════════════════════════════════════════════════════════════
// Mock API responses
// ═══════════════════════════════════════════════════════════════════════════════

const mockSettings = {
  maxConcurrentTasks: 50,
  defaultAgentModel: 'gpt-4o',
  approvalThreshold: 1.0,
  autoRetryEnabled: true,
  maxRetries: 5,
  logLevel: 'info',
}

const mockApiKeys = [
  {
    id: 'key-1',
    provider: 'openai',
    maskedKey: 'sk-****abcd',
    createdAt: '2025-12-01T00:00:00Z',
  },
  {
    id: 'key-2',
    provider: 'anthropic',
    maskedKey: 'sk-ant-****efgh',
    createdAt: '2025-12-15T00:00:00Z',
  },
]

// ═══════════════════════════════════════════════════════════════════════════════
// Settings Page Tests
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Settings Page', () => {
  test.beforeEach(async ({ page }) => {
    // Mock all settings-related API endpoints
    await page.route('**/api/v1/settings', async (route) => {
      if (route.request().method() === 'GET') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: mockSettings }),
        })
      } else if (route.request().method() === 'PUT') {
        const body = route.request().postDataJSON()
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: { ...mockSettings, ...body } }),
        })
      }
    })

    await page.route('**/api/v1/settings/api-keys', async (route) => {
      if (route.request().method() === 'GET') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: mockApiKeys }),
        })
      } else if (route.request().method() === 'POST') {
        const body = route.request().postDataJSON()
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            data: {
              id: `key-${Date.now()}`,
              provider: body.provider,
              maskedKey: `****${body.key.slice(-4)}`,
              createdAt: new Date().toISOString(),
            },
          }),
        })
      }
    })

    await page.route('**/api/v1/settings/api-keys/*', async (route) => {
      if (route.request().method() === 'DELETE') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: { id: 'key-1', status: 'revoked' } }),
        })
      }
    })

    await page.route('**/health', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          status: 'healthy',
          version: '1.0.0',
          uptime: 3600,
          services: {},
        }),
      })
    })

    await page.goto('/settings')
  })

  // ── Page Load ──────────────────────────────────────────────────────────────

  test.describe('Page Load and Layout', () => {
    test('should display settings sidebar with all sections', async ({ page }) => {
      await expect(page.locator('button', { hasText: 'General' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'API Keys' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'Resource Limits' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'Notifications' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'Appearance' })).toBeVisible()
    })

    test('should default to General section', async ({ page }) => {
      await expect(page.locator('h2', { hasText: 'General Settings' })).toBeVisible()
    })

    test('should load settings from backend', async ({ page }) => {
      const concurrentInput = page.locator('input[type="number"]').first()
      await expect(concurrentInput).toHaveValue('50')
    })
  })

  // ── Section Navigation ─────────────────────────────────────────────────────

  test.describe('Section Navigation', () => {
    test('should navigate to API Keys section', async ({ page }) => {
      await page.locator('button', { hasText: 'API Keys' }).click()
      await expect(page.locator('h2', { hasText: 'API Keys' })).toBeVisible()
    })

    test('should navigate to Resource Limits section', async ({ page }) => {
      await page.locator('button', { hasText: 'Resource Limits' }).click()
      await expect(page.locator('h2', { hasText: 'Resource Limits' })).toBeVisible()
    })

    test('should navigate to Notifications section', async ({ page }) => {
      await page.locator('button', { hasText: 'Notifications' }).click()
      await expect(page.locator('h2', { hasText: 'Notifications' })).toBeVisible()
    })

    test('should navigate to Appearance section', async ({ page }) => {
      await page.locator('button', { hasText: 'Appearance' }).click()
      await expect(page.locator('h2', { hasText: 'Appearance' })).toBeVisible()
    })

    test('should highlight active section in sidebar', async ({ page }) => {
      const apiKeysBtn = page.locator('button', { hasText: 'API Keys' })
      await apiKeysBtn.click()
      await expect(apiKeysBtn).toHaveClass(/bg-apex-accent-primary/)
    })
  })

  // ── General Settings ───────────────────────────────────────────────────────

  test.describe('General Settings', () => {
    test('should display max concurrent agents input', async ({ page }) => {
      await expect(page.locator('label', { hasText: 'Max Concurrent Agents' })).toBeVisible()
    })

    test('should display default model selector', async ({ page }) => {
      await expect(page.locator('label', { hasText: 'Default Model' })).toBeVisible()
      const select = page.locator('select')
      await expect(select).toBeVisible()
    })

    test('should display model routing toggle', async ({ page }) => {
      await expect(page.locator('text=Enable Model Routing')).toBeVisible()
    })

    test('should save general settings', async ({ page }) => {
      // Click save button
      await page.locator('button', { hasText: 'Save Changes' }).click()

      // Should show success toast
      await expect(page.locator('text=Settings saved')).toBeVisible({ timeout: 5000 })
    })

    test('should show saving state during save', async ({ page }) => {
      // Add delay to PUT response
      await page.route('**/api/v1/settings', async (route) => {
        if (route.request().method() === 'PUT') {
          await new Promise((resolve) => setTimeout(resolve, 500))
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ data: mockSettings }),
          })
        } else {
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ data: mockSettings }),
          })
        }
      })

      await page.locator('button', { hasText: 'Save Changes' }).click()
      await expect(page.locator('text=Saving...')).toBeVisible()
    })
  })

  // ── API Keys ───────────────────────────────────────────────────────────────

  test.describe('API Keys Management', () => {
    test.beforeEach(async ({ page }) => {
      await page.locator('button', { hasText: 'API Keys' }).click()
    })

    test('should display existing API keys', async ({ page }) => {
      await expect(page.locator('text=openai')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=anthropic')).toBeVisible()
      await expect(page.locator('text=sk-****abcd')).toBeVisible()
      await expect(page.locator('text=sk-ant-****efgh')).toBeVisible()
    })

    test('should show Add Key button', async ({ page }) => {
      await expect(page.locator('button', { hasText: 'Add Key' })).toBeVisible()
    })

    test('should toggle add key form', async ({ page }) => {
      // Click Add Key to show form
      await page.locator('button', { hasText: 'Add Key' }).click()
      await expect(page.locator('text=Select provider...')).toBeVisible()
      await expect(page.locator('input[placeholder="sk-..."]')).toBeVisible()

      // Click Add Key again to hide form
      await page.locator('button', { hasText: 'Add Key' }).click()
      await expect(page.locator('text=Select provider...')).not.toBeVisible()
    })

    test('should show provider dropdown options', async ({ page }) => {
      await page.locator('button', { hasText: 'Add Key' }).click()
      const select = page.locator('select').last()

      await expect(select.locator('option', { hasText: 'OpenAI' })).toBeAttached()
      await expect(select.locator('option', { hasText: 'Anthropic' })).toBeAttached()
      await expect(select.locator('option', { hasText: 'Google AI' })).toBeAttached()
      await expect(select.locator('option', { hasText: 'Cohere' })).toBeAttached()
      await expect(select.locator('option', { hasText: 'Mistral' })).toBeAttached()
    })

    test('should add a new API key', async ({ page }) => {
      await page.locator('button', { hasText: 'Add Key' }).click()

      // Select provider
      await page.locator('select').last().selectOption('openai')

      // Enter key
      await page.locator('input[placeholder="sk-..."]').fill('sk-test1234abcd')

      // Click Save Key
      await page.locator('button', { hasText: 'Save Key' }).click()

      // Should show success toast
      await expect(page.locator('text=API key for openai added')).toBeVisible({ timeout: 5000 })
    })

    test('should show error when adding key without provider', async ({ page }) => {
      await page.locator('button', { hasText: 'Add Key' }).click()

      // Enter key but no provider
      await page.locator('input[placeholder="sk-..."]').fill('sk-test1234')

      // Save Key should be disabled
      const saveBtn = page.locator('button', { hasText: 'Save Key' })
      await expect(saveBtn).toBeDisabled()
    })

    test('should cancel add key form', async ({ page }) => {
      await page.locator('button', { hasText: 'Add Key' }).click()

      // Fill some data
      await page.locator('select').last().selectOption('openai')
      await page.locator('input[placeholder="sk-..."]').fill('sk-test')

      // Click Cancel
      await page.locator('button', { hasText: 'Cancel' }).click()

      // Form should be hidden
      await expect(page.locator('input[placeholder="sk-..."]')).not.toBeVisible()
    })

    test('should revoke an API key', async ({ page }) => {
      // Wait for keys to load
      await expect(page.locator('text=openai')).toBeVisible({ timeout: 5000 })

      // Click revoke button (Trash icon) on first key
      await page.locator('button[title="Revoke key"]').first().click()

      // Should show success toast
      await expect(page.locator('text=revoked')).toBeVisible({ timeout: 5000 })
    })

    test('should toggle key visibility in input', async ({ page }) => {
      await page.locator('button', { hasText: 'Add Key' }).click()

      const keyInput = page.locator('input[placeholder="sk-..."]')
      // Default should be password type
      await expect(keyInput).toHaveAttribute('type', 'password')

      // Click eye icon to show
      await page.locator('.relative button').click()
      await expect(keyInput).toHaveAttribute('type', 'text')

      // Click again to hide
      await page.locator('.relative button').click()
      await expect(keyInput).toHaveAttribute('type', 'password')
    })

    test('should not show save button in API Keys section', async ({ page }) => {
      await expect(page.locator('button', { hasText: 'Save Changes' })).not.toBeVisible()
    })

    test('should show empty state when no keys', async ({ page }) => {
      // Override route to return empty keys
      await page.route('**/api/v1/settings/api-keys', async (route) => {
        if (route.request().method() === 'GET') {
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ data: [] }),
          })
        }
      })

      await page.reload()
      await page.locator('button', { hasText: 'API Keys' }).click()

      await expect(page.locator('text=No API keys configured')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Add a key to connect to LLM providers')).toBeVisible()
    })
  })

  // ── Resource Limits ────────────────────────────────────────────────────────

  test.describe('Resource Limits', () => {
    test.beforeEach(async ({ page }) => {
      await page.locator('button', { hasText: 'Resource Limits' }).click()
    })

    test('should display all limit inputs', async ({ page }) => {
      await expect(page.locator('label', { hasText: 'Default Token Limit' })).toBeVisible()
      await expect(page.locator('label', { hasText: 'Default Cost Limit ($)' })).toBeVisible()
      await expect(page.locator('label', { hasText: 'Default Time Limit (seconds)' })).toBeVisible()
      await expect(page.locator('label', { hasText: 'Circuit Breaker Threshold' })).toBeVisible()
    })

    test('should show circuit breaker description', async ({ page }) => {
      await expect(
        page.locator('text=Number of consecutive failures before circuit breaks')
      ).toBeVisible()
    })

    test('should show save button', async ({ page }) => {
      await expect(page.locator('button', { hasText: 'Save Changes' })).toBeVisible()
    })
  })

  // ── Notifications ──────────────────────────────────────────────────────────

  test.describe('Notifications', () => {
    test.beforeEach(async ({ page }) => {
      await page.locator('button', { hasText: 'Notifications' }).click()
    })

    test('should display notification toggles', async ({ page }) => {
      await expect(page.locator('text=Notify on Task Failure')).toBeVisible()
      await expect(page.locator('text=Cost Threshold Alerts')).toBeVisible()
    })

    test('should show cost threshold input when enabled', async ({ page }) => {
      await expect(page.locator('label', { hasText: 'Cost Threshold ($)' })).toBeVisible()
    })

    test('should hide cost threshold input when toggle disabled', async ({ page }) => {
      // Click cost threshold toggle to disable
      const toggles = page.locator('.rounded-full.transition-colors')
      await toggles.nth(1).click()

      // Cost threshold input should be hidden
      await expect(page.locator('label', { hasText: 'Cost Threshold ($)' })).not.toBeVisible()
    })
  })

  // ── Appearance ─────────────────────────────────────────────────────────────

  test.describe('Appearance', () => {
    test.beforeEach(async ({ page }) => {
      await page.locator('button', { hasText: 'Appearance' }).click()
    })

    test('should display theme options', async ({ page }) => {
      await expect(page.locator('button', { hasText: 'dark' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'light' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'system' })).toBeVisible()
    })

    test('should highlight active theme', async ({ page }) => {
      // Dark should be active by default
      await expect(page.locator('button', { hasText: 'dark' })).toHaveClass(
        /bg-apex-accent-primary/
      )
    })

    test('should switch themes', async ({ page }) => {
      await page.locator('button', { hasText: 'light' }).click()
      await expect(page.locator('button', { hasText: 'light' })).toHaveClass(
        /bg-apex-accent-primary/
      )
      await expect(page.locator('button', { hasText: 'dark' })).not.toHaveClass(
        /bg-apex-accent-primary/
      )
    })

    test('should display compact mode toggle', async ({ page }) => {
      await expect(page.locator('text=Compact Mode')).toBeVisible()
      await expect(page.locator('text=Reduce spacing for more information density')).toBeVisible()
    })
  })

  // ── Error Handling ─────────────────────────────────────────────────────────

  test.describe('Error Handling', () => {
    test('should show error toast on save failure', async ({ page }) => {
      await page.route('**/api/v1/settings', async (route) => {
        if (route.request().method() === 'PUT') {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ code: 'INTERNAL_ERROR', message: 'Server error' }),
          })
        } else {
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ data: mockSettings }),
          })
        }
      })

      await page.locator('button', { hasText: 'Save Changes' }).click()
      await expect(page.locator('text=Failed to save settings')).toBeVisible({ timeout: 5000 })
    })

    test('should show error toast on API key creation failure', async ({ page }) => {
      await page.route('**/api/v1/settings/api-keys', async (route) => {
        if (route.request().method() === 'POST') {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ code: 'INTERNAL_ERROR', message: 'Server error' }),
          })
        } else {
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ data: mockApiKeys }),
          })
        }
      })

      await page.locator('button', { hasText: 'API Keys' }).click()
      await page.locator('button', { hasText: 'Add Key' }).click()
      await page.locator('select').last().selectOption('openai')
      await page.locator('input[placeholder="sk-..."]').fill('sk-test1234abcd')
      await page.locator('button', { hasText: 'Save Key' }).click()

      await expect(page.locator('text=Failed to add API key')).toBeVisible({ timeout: 5000 })
    })

    test('should show error toast on API key revoke failure', async ({ page }) => {
      await page.route('**/api/v1/settings/api-keys/*', async (route) => {
        if (route.request().method() === 'DELETE') {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ code: 'INTERNAL_ERROR', message: 'Server error' }),
          })
        }
      })

      await page.locator('button', { hasText: 'API Keys' }).click()
      await expect(page.locator('text=openai')).toBeVisible({ timeout: 5000 })

      await page.locator('button[title="Revoke key"]').first().click()
      await expect(page.locator('text=Failed to revoke API key')).toBeVisible({ timeout: 5000 })
    })
  })
})
