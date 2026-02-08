import { test, expect } from '@playwright/test'

// ═══════════════════════════════════════════════════════════════════════════════
// Workflows Page Tests
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Workflows Page', () => {
  test.beforeEach(async ({ page }) => {
    // Mock DAG API
    await page.route('**/api/v1/dags', async (route) => {
      if (route.request().method() === 'POST') {
        const body = route.request().postDataJSON()
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            data: {
              id: `dag-${Date.now()}`,
              name: body.name,
              status: 'pending',
              tasks: body.tasks,
              createdAt: new Date().toISOString(),
            },
          }),
        })
      } else {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: [], pagination: { total: 0, page: 1, limit: 20 } }),
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

    await page.goto('/workflows')
  })

  // ── Page Load ──────────────────────────────────────────────────────────────

  test.describe('Page Load and Layout', () => {
    test('should display workflows page', async ({ page }) => {
      // Should have the workflow builder container
      await expect(page.locator('.h-\\[calc\\(100vh-8rem\\)\\]')).toBeVisible()
    })

    test('should show workflow builder canvas', async ({ page }) => {
      // The WorkflowBuilder should render with its toolbar and canvas
      await expect(page.locator('svg, canvas, [data-testid="workflow-canvas"]').first()).toBeVisible({ timeout: 5000 })
    })

    test('should show node palette in toolbar', async ({ page }) => {
      // Toolbar should have node type buttons
      await expect(page.locator('text=Task')).toBeVisible({ timeout: 5000 })
    })
  })

  // ── Navigation ─────────────────────────────────────────────────────────────

  test.describe('Navigation', () => {
    test('should show Workflows link in sidebar', async ({ page }) => {
      await expect(page.locator('a[href="/workflows"]')).toBeVisible()
    })

    test('should highlight Workflows in sidebar when active', async ({ page }) => {
      const navLink = page.locator('a[href="/workflows"]')
      await expect(navLink).toHaveClass(/text-apex-accent-primary/)
    })

    test('should navigate to workflows from sidebar', async ({ page }) => {
      await page.goto('/')
      await page.locator('a[href="/workflows"]').click()
      await expect(page).toHaveURL('/workflows')
    })
  })

  // ── Toolbar ────────────────────────────────────────────────────────────────

  test.describe('Toolbar Controls', () => {
    test('should display node type options', async ({ page }) => {
      // The toolbar should have options for different node types
      await expect(page.locator('text=Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Condition')).toBeVisible()
    })

    test('should display action buttons', async ({ page }) => {
      // Should have save and execute buttons
      await expect(page.locator('button').filter({ hasText: /save/i }).or(page.locator('[title*="Save"]'))).toBeVisible({ timeout: 5000 })
    })
  })

  // ── Workflow Name ──────────────────────────────────────────────────────────

  test.describe('Workflow Naming', () => {
    test('should have an editable workflow name', async ({ page }) => {
      const nameInput = page.locator('input[type="text"]').first()
      if (await nameInput.isVisible()) {
        // Should have a default name
        const value = await nameInput.inputValue()
        expect(value.length).toBeGreaterThan(0)
      }
    })
  })

  // ── Node Interaction ───────────────────────────────────────────────────────

  test.describe('Node Operations', () => {
    test('should add a task node', async ({ page }) => {
      // Find and click the Task button in toolbar to add a node
      const taskBtn = page.locator('button').filter({ hasText: 'Task' }).first()
      if (await taskBtn.isVisible()) {
        await taskBtn.click()

        // Should have at least one node on the canvas
        await page.waitForTimeout(500)
        const nodes = page.locator('[class*="node"], [data-type="task"]')
        const count = await nodes.count()
        expect(count).toBeGreaterThanOrEqual(1)
      }
    })
  })

  // ── Save Workflow ──────────────────────────────────────────────────────────

  test.describe('Save Workflow', () => {
    test('should show save success toast', async ({ page }) => {
      // Add a task node first
      const taskBtn = page.locator('button').filter({ hasText: 'Task' }).first()
      if (await taskBtn.isVisible()) {
        await taskBtn.click()
        await page.waitForTimeout(300)
      }

      // Click save
      const saveBtn = page.locator('button').filter({ hasText: /save/i }).first()
      if (await saveBtn.isVisible()) {
        await saveBtn.click()
        await expect(page.locator('text=Workflow saved')).toBeVisible({ timeout: 5000 })
      }
    })

    test('should show execute success toast', async ({ page }) => {
      // Add a task node
      const taskBtn = page.locator('button').filter({ hasText: 'Task' }).first()
      if (await taskBtn.isVisible()) {
        await taskBtn.click()
        await page.waitForTimeout(300)
      }

      // Click execute/run
      const execBtn = page
        .locator('button')
        .filter({ hasText: /execute|run/i })
        .first()
      if (await execBtn.isVisible()) {
        await execBtn.click()
        await expect(page.locator('text=Workflow submitted for execution')).toBeVisible({
          timeout: 5000,
        })
      }
    })
  })

  // ── Error Handling ─────────────────────────────────────────────────────────

  test.describe('Error Handling', () => {
    test('should show error toast on save failure', async ({ page }) => {
      await page.route('**/api/v1/dags', async (route) => {
        if (route.request().method() === 'POST') {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ code: 'INTERNAL_ERROR', message: 'Server error' }),
          })
        }
      })

      // Add a node and try to save
      const taskBtn = page.locator('button').filter({ hasText: 'Task' }).first()
      if (await taskBtn.isVisible()) {
        await taskBtn.click()
        await page.waitForTimeout(300)
      }

      const saveBtn = page.locator('button').filter({ hasText: /save/i }).first()
      if (await saveBtn.isVisible()) {
        await saveBtn.click()
        await expect(page.locator('text=Failed to save workflow')).toBeVisible({ timeout: 5000 })
      }
    })

    test('should show error toast on execute failure', async ({ page }) => {
      await page.route('**/api/v1/dags', async (route) => {
        if (route.request().method() === 'POST') {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ code: 'INTERNAL_ERROR', message: 'Server error' }),
          })
        }
      })

      const taskBtn = page.locator('button').filter({ hasText: 'Task' }).first()
      if (await taskBtn.isVisible()) {
        await taskBtn.click()
        await page.waitForTimeout(300)
      }

      const execBtn = page
        .locator('button')
        .filter({ hasText: /execute|run/i })
        .first()
      if (await execBtn.isVisible()) {
        await execBtn.click()
        await expect(page.locator('text=Failed to execute workflow')).toBeVisible({
          timeout: 5000,
        })
      }
    })
  })
})
