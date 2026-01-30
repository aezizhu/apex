import {
  test,
  expect,
  createMockAgent,
  createMockTask,
  createMockMetrics,
  MockAgent,
  MockTask,
} from './fixtures'

test.describe('Dashboard Page', () => {
  test.describe('Page Load and Layout', () => {
    test('should display dashboard header with title and live indicator', async ({
      page,
      wsMock,
    }) => {
      await page.goto('/')

      // Check page title
      await expect(page.locator('h1')).toHaveText('Dashboard')
      await expect(page.locator('text=Real-time overview of your agent swarm')).toBeVisible()

      // Check live indicator
      await expect(page.locator('text=Live')).toBeVisible()
      const liveIndicator = page.locator('.animate-pulse')
      await expect(liveIndicator).toBeVisible()
    })

    test('should display all stat cards', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send initial metrics
      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          activeAgents: 25,
          runningTasks: 10,
          totalCost: 250.5,
          totalTokens: 5000000,
        })
      )

      // Verify stat cards are visible
      await expect(page.locator('text=Active Agents')).toBeVisible()
      await expect(page.locator('text=Running Tasks')).toBeVisible()
      await expect(page.locator('text=Total Cost')).toBeVisible()
      await expect(page.locator('text=Total Tokens')).toBeVisible()
    })

    test('should display sidebar navigation', async ({ page }) => {
      await page.goto('/')

      // Check navigation links
      await expect(page.locator('nav').getByRole('link', { name: /dashboard/i })).toBeVisible()
      await expect(page.locator('nav').getByRole('link', { name: /agents/i })).toBeVisible()
      await expect(page.locator('nav').getByRole('link', { name: /tasks/i })).toBeVisible()
      await expect(page.locator('nav').getByRole('link', { name: /approvals/i })).toBeVisible()
      await expect(page.locator('nav').getByRole('link', { name: /settings/i })).toBeVisible()
    })
  })

  test.describe('Metrics Display', () => {
    test('should update metrics in real-time via WebSocket', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send initial metrics
      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          activeAgents: 10,
          runningTasks: 5,
          totalCost: 100.0,
          totalTokens: 1000000,
        })
      )

      // Wait for initial values
      await expect(page.locator('text=10').first()).toBeVisible({ timeout: 5000 })

      // Send updated metrics
      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          activeAgents: 25,
          runningTasks: 15,
          totalCost: 500.0,
          totalTokens: 5000000,
        })
      )

      // Verify updates
      await expect(page.locator('text=25').first()).toBeVisible({ timeout: 5000 })
    })

    test('should display formatted cost values', async ({ page, wsMock }) => {
      await page.goto('/')

      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          totalCost: 1234.56,
        })
      )

      // Check formatted cost (should show as $1,234.56 or similar)
      await expect(page.locator('text=/\\$1,?234\\.56/')).toBeVisible({ timeout: 5000 })
    })

    test('should display formatted token values', async ({ page, wsMock }) => {
      await page.goto('/')

      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          totalTokens: 5000000,
        })
      )

      // Check formatted tokens (should show as 5M or 5,000,000)
      await expect(page.locator('text=/5M|5,?000,?000/')).toBeVisible({ timeout: 5000 })
    })

    test("should display today's summary with correct values", async ({ page, wsMock }) => {
      await page.goto('/')

      await wsMock.sendMetricsUpdate(
        createMockMetrics({
          avgLatencyMs: 150,
          successRate: 0.95,
        })
      )

      // Check Today's Summary section
      await expect(page.locator("text=Today's Summary")).toBeVisible()
      await expect(page.locator('text=Completed')).toBeVisible()
      await expect(page.locator('text=Avg. Latency')).toBeVisible()
      await expect(page.locator('text=Success Rate')).toBeVisible()

      // Verify success rate display (95.0%)
      await expect(page.locator('text=/95\\.0%/')).toBeVisible({ timeout: 5000 })
    })

    test('should color-code success rate based on thresholds', async ({ page, wsMock }) => {
      await page.goto('/')

      // High success rate (green)
      await wsMock.sendMetricsUpdate(createMockMetrics({ successRate: 0.98 }))
      await expect(page.locator('text=/98\\.0%/').first()).toHaveClass(/text-green-500/, {
        timeout: 5000,
      })

      // Medium success rate (yellow)
      await wsMock.sendMetricsUpdate(createMockMetrics({ successRate: 0.85 }))
      await expect(page.locator('text=/85\\.0%/').first()).toHaveClass(/text-yellow-500/, {
        timeout: 5000,
      })

      // Low success rate (red)
      await wsMock.sendMetricsUpdate(createMockMetrics({ successRate: 0.7 }))
      await expect(page.locator('text=/70\\.0%/').first()).toHaveClass(/text-red-500/, {
        timeout: 5000,
      })
    })
  })

  test.describe('Agent Swarm Grid', () => {
    test('should display Agent Swarm section', async ({ page }) => {
      await page.goto('/')

      await expect(page.locator('text=Agent Swarm')).toBeVisible()
    })

    test('should display agents received via WebSocket', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send multiple agents
      const agents: MockAgent[] = [
        createMockAgent({ id: 'agent-1', name: 'Alpha', status: 'busy' }),
        createMockAgent({ id: 'agent-2', name: 'Beta', status: 'idle' }),
        createMockAgent({ id: 'agent-3', name: 'Gamma', status: 'error' }),
      ]

      for (const agent of agents) {
        await wsMock.sendAgentUpdate(agent)
      }

      // Verify agent count display
      await expect(page.locator('text=/3 agents/')).toBeVisible({ timeout: 5000 })
    })

    test('should update busy agent count', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send agents with different statuses
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', status: 'busy' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a2', status: 'busy' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a3', status: 'idle' }))

      // Check busy count in header
      await expect(page.locator('text=/2 busy/')).toBeVisible({ timeout: 5000 })
    })

    test('should display agent status legend', async ({ page }) => {
      await page.goto('/')

      // Check legend items
      await expect(page.locator('text=Agent Status')).toBeVisible()
      await expect(page.locator('.glass >> text=Busy')).toBeVisible()
      await expect(page.locator('.glass >> text=Idle')).toBeVisible()
      await expect(page.locator('.glass >> text=Error')).toBeVisible()
      await expect(page.locator('.glass >> text=Paused')).toBeVisible()
    })

    test('should show agent hover card on mouse over', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send an agent
      const agent = createMockAgent({
        id: 'hover-agent',
        name: 'HoverTest',
        model: 'gpt-4-turbo',
        status: 'busy',
        currentLoad: 5,
        maxLoad: 10,
        successRate: 0.92,
        totalTokens: 100000,
        totalCost: 5.25,
      })
      await wsMock.sendAgentUpdate(agent)

      // Wait for agent to appear
      await page.waitForSelector('svg', { timeout: 5000 })

      // Hover over the agent hexagon
      const agentHex = page.locator('svg').first()
      await agentHex.hover()

      // Wait for hover card
      await expect(page.locator('text=HoverTest')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=gpt-4-turbo')).toBeVisible()
      await expect(page.locator('text=/5\\/10/')).toBeVisible()
      await expect(page.locator('text=/92\\.0%/')).toBeVisible()
    })
  })

  test.describe('Recent Tasks Section', () => {
    test('should display Recent Tasks section', async ({ page }) => {
      await page.goto('/')

      await expect(page.locator('text=Recent Tasks')).toBeVisible()
    })

    test('should show "No recent tasks" when empty', async ({ page }) => {
      await page.goto('/')

      await expect(page.locator('text=No recent tasks')).toBeVisible()
    })

    test('should display tasks received via WebSocket', async ({ page, wsMock }) => {
      await page.goto('/')

      const task = createMockTask({
        id: 'task-1',
        name: 'Process Data',
        status: 'completed',
        tokensUsed: 5000,
        costDollars: 0.25,
      })
      await wsMock.sendTaskUpdate(task)

      await expect(page.locator('text=Process Data')).toBeVisible({ timeout: 5000 })
    })

    test('should show task status badges with correct colors', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send tasks with different statuses
      const tasks: MockTask[] = [
        createMockTask({ id: 't1', name: 'Completed Task', status: 'completed' }),
        createMockTask({ id: 't2', name: 'Running Task', status: 'running' }),
        createMockTask({ id: 't3', name: 'Failed Task', status: 'failed' }),
        createMockTask({ id: 't4', name: 'Pending Task', status: 'pending' }),
      ]

      for (const task of tasks) {
        await wsMock.sendTaskUpdate(task)
      }

      // Wait for tasks to appear
      await expect(page.locator('text=Completed Task')).toBeVisible({ timeout: 5000 })

      // Check status badges
      await expect(page.locator('text=completed').first()).toHaveClass(/text-green-500/)
      await expect(page.locator('text=running').first()).toHaveClass(/text-blue-500/)
      await expect(page.locator('text=failed').first()).toHaveClass(/text-red-500/)
      await expect(page.locator('text=pending').first()).toHaveClass(/text-yellow-500/)
    })

    test('should limit displayed tasks to 5 most recent', async ({ page, wsMock }) => {
      await page.goto('/')

      // Send 7 tasks
      for (let i = 0; i < 7; i++) {
        await wsMock.sendTaskUpdate(
          createMockTask({
            id: `task-${i}`,
            name: `Task ${i}`,
            createdAt: new Date(Date.now() - i * 60000).toISOString(),
          })
        )
      }

      // Wait for tasks to render
      await page.waitForTimeout(500)

      // Count task items in Recent Tasks section
      const recentTasksSection = page.locator('text=Recent Tasks').locator('..').locator('..')
      const taskItems = recentTasksSection.locator('[class*="border-b"]')

      // Should only show 5 tasks max
      await expect(taskItems).toHaveCount(5, { timeout: 5000 })
    })
  })

  test.describe('Performance Trends Chart', () => {
    test('should display Performance Trends section', async ({ page }) => {
      await page.goto('/')

      await expect(page.locator('text=Performance Trends')).toBeVisible()
    })
  })

  test.describe('Navigation', () => {
    test('should navigate to Agents page', async ({ page }) => {
      await page.goto('/')

      await page.locator('nav').getByRole('link', { name: /agents/i }).click()
      await expect(page).toHaveURL('/agents')
      await expect(page.locator('h1')).toHaveText('Agents')
    })

    test('should navigate to Tasks page', async ({ page }) => {
      await page.goto('/')

      await page.locator('nav').getByRole('link', { name: /tasks/i }).click()
      await expect(page).toHaveURL('/tasks')
      await expect(page.locator('h1')).toHaveText('Tasks')
    })

    test('should navigate to Approvals page', async ({ page }) => {
      await page.goto('/')

      await page.locator('nav').getByRole('link', { name: /approvals/i }).click()
      await expect(page).toHaveURL('/approvals')
      await expect(page.locator('h1')).toHaveText('Approval Queue')
    })

    test('should navigate to Settings page', async ({ page }) => {
      await page.goto('/')

      await page.locator('nav').getByRole('link', { name: /settings/i }).click()
      await expect(page).toHaveURL('/settings')
    })
  })

  test.describe('Error States', () => {
    test('should handle WebSocket disconnection gracefully', async ({ page }) => {
      await page.goto('/')

      // Simulate WebSocket close
      await page.evaluate(() => {
        const ws = (window as unknown as { __mockWs?: { close: () => void } }).__mockWs
        if (ws) {
          ws.close()
        }
      })

      // Page should still be functional, showing last known data
      await expect(page.locator('h1')).toHaveText('Dashboard')
    })
  })

  test.describe('Responsive Layout', () => {
    test('should adapt stat cards to mobile viewport', async ({ page, wsMock }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/')

      // Stat cards should stack vertically on mobile
      await expect(page.locator('text=Active Agents')).toBeVisible()
      await expect(page.locator('text=Running Tasks')).toBeVisible()
    })

    test('should collapse sidebar on small screens', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/')

      // The layout should adjust for mobile
      await expect(page.locator('h1')).toHaveText('Dashboard')
    })
  })
})
