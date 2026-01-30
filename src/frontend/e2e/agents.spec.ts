import { test, expect, createMockAgent, MockAgent } from './fixtures'

test.describe('Agents Page', () => {
  test.describe('Page Load and Layout', () => {
    test('should display agents page header', async ({ page }) => {
      await page.goto('/agents')

      await expect(page.locator('h1')).toHaveText('Agents')
      await expect(page.locator('text=Manage and monitor your agent swarm')).toBeVisible()
    })

    test('should display Register Agent button', async ({ page }) => {
      await page.goto('/agents')

      const registerButton = page.locator('button', { hasText: 'Register Agent' })
      await expect(registerButton).toBeVisible()
    })

    test('should display search input', async ({ page }) => {
      await page.goto('/agents')

      const searchInput = page.locator('input[placeholder="Search agents..."]')
      await expect(searchInput).toBeVisible()
    })
  })

  test.describe('Status Filters', () => {
    test('should display all status filter buttons', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send some agents to populate counts
      const agents: MockAgent[] = [
        createMockAgent({ id: 'a1', status: 'idle' }),
        createMockAgent({ id: 'a2', status: 'busy' }),
        createMockAgent({ id: 'a3', status: 'busy' }),
        createMockAgent({ id: 'a4', status: 'error' }),
        createMockAgent({ id: 'a5', status: 'paused' }),
      ]
      for (const agent of agents) {
        await wsMock.sendAgentUpdate(agent)
      }

      // Check filter buttons exist
      await expect(page.locator('button', { hasText: /^All/ })).toBeVisible()
      await expect(page.locator('button', { hasText: /^Idle/ })).toBeVisible()
      await expect(page.locator('button', { hasText: /^Busy/ })).toBeVisible()
      await expect(page.locator('button', { hasText: /^Error/ })).toBeVisible()
      await expect(page.locator('button', { hasText: /^Paused/ })).toBeVisible()
    })

    test('should show agent counts in filter buttons', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send agents with specific statuses
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', status: 'idle' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a2', status: 'busy' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a3', status: 'busy' }))

      // Check counts appear in buttons
      await expect(page.locator('button', { hasText: /All.*\(3\)/ })).toBeVisible({ timeout: 5000 })
      await expect(page.locator('button', { hasText: /Idle.*\(1\)/ })).toBeVisible()
      await expect(page.locator('button', { hasText: /Busy.*\(2\)/ })).toBeVisible()
    })

    test('should filter agents by status when clicking filter button', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send agents
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', name: 'IdleAgent', status: 'idle' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a2', name: 'BusyAgent', status: 'busy' }))
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a3', name: 'ErrorAgent', status: 'error' })
      )

      // Switch to list view to verify filtering
      await page.locator('button').filter({ has: page.locator('svg') }).last().click()

      // Click Busy filter
      await page.locator('button', { hasText: /^Busy/ }).click()

      // Should only show busy agents
      await expect(page.locator('text=BusyAgent')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=IdleAgent')).not.toBeVisible()
      await expect(page.locator('text=ErrorAgent')).not.toBeVisible()

      // Click All filter
      await page.locator('button', { hasText: /^All/ }).click()

      // Should show all agents again
      await expect(page.locator('text=IdleAgent')).toBeVisible()
      await expect(page.locator('text=BusyAgent')).toBeVisible()
      await expect(page.locator('text=ErrorAgent')).toBeVisible()
    })

    test('should highlight active filter button', async ({ page }) => {
      await page.goto('/agents')

      // All should be active by default
      const allButton = page.locator('button', { hasText: /^All/ })
      await expect(allButton).toHaveClass(/bg-apex-accent-primary/)

      // Click Busy filter
      await page.locator('button', { hasText: /^Busy/ }).click()
      const busyButton = page.locator('button', { hasText: /^Busy/ })
      await expect(busyButton).toHaveClass(/bg-apex-accent-primary/)
      await expect(allButton).not.toHaveClass(/bg-apex-accent-primary/)
    })
  })

  test.describe('Search Functionality', () => {
    test('should filter agents by search query', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send agents
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', name: 'Alpha Agent' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a2', name: 'Beta Agent' }))
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a3', name: 'Gamma Agent' }))

      // Switch to list view
      await page.locator('button').filter({ has: page.locator('svg') }).last().click()

      // Search for "Alpha"
      await page.fill('input[placeholder="Search agents..."]', 'Alpha')

      // Should only show Alpha Agent
      await expect(page.locator('text=Alpha Agent')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Beta Agent')).not.toBeVisible()
      await expect(page.locator('text=Gamma Agent')).not.toBeVisible()
    })

    test('should be case-insensitive in search', async ({ page, wsMock }) => {
      await page.goto('/agents')

      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', name: 'TestAgent' }))

      // Switch to list view
      await page.locator('button').filter({ has: page.locator('svg') }).last().click()

      // Search with lowercase
      await page.fill('input[placeholder="Search agents..."]', 'testagent')

      await expect(page.locator('text=TestAgent')).toBeVisible({ timeout: 5000 })
    })

    test('should combine search with status filter', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send agents
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a1', name: 'Alpha Busy', status: 'busy' })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a2', name: 'Alpha Idle', status: 'idle' })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a3', name: 'Beta Busy', status: 'busy' })
      )

      // Switch to list view
      await page.locator('button').filter({ has: page.locator('svg') }).last().click()

      // Apply status filter
      await page.locator('button', { hasText: /^Busy/ }).click()

      // Apply search
      await page.fill('input[placeholder="Search agents..."]', 'Alpha')

      // Should only show Alpha Busy
      await expect(page.locator('text=Alpha Busy')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Alpha Idle')).not.toBeVisible()
      await expect(page.locator('text=Beta Busy')).not.toBeVisible()
    })
  })

  test.describe('View Toggle', () => {
    test('should display view toggle buttons', async ({ page }) => {
      await page.goto('/agents')

      // Grid and List view buttons should be visible
      const toggleButtons = page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button')
      await expect(toggleButtons).toHaveCount(2)
    })

    test('should default to grid view', async ({ page, wsMock }) => {
      await page.goto('/agents')

      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1' }))

      // Grid view container should be visible
      await expect(page.locator('.h-\\[600px\\]')).toBeVisible()
    })

    test('should switch to list view', async ({ page, wsMock }) => {
      await page.goto('/agents')

      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', name: 'ListViewAgent' }))

      // Click list view button (second button in toggle)
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()

      // Should show table headers
      await expect(page.locator('text=Agent').first()).toBeVisible()
      await expect(page.locator('text=Model')).toBeVisible()
      await expect(page.locator('text=Status')).toBeVisible()
      await expect(page.locator('text=Load')).toBeVisible()
      await expect(page.locator('text=Success')).toBeVisible()
      await expect(page.locator('text=Cost')).toBeVisible()
    })
  })

  test.describe('Agent Grid View', () => {
    test('should display agent count', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Send multiple agents
      for (let i = 0; i < 5; i++) {
        await wsMock.sendAgentUpdate(createMockAgent({ id: `agent-${i}` }))
      }

      await expect(page.locator('text=/5 agents/')).toBeVisible({ timeout: 5000 })
    })

    test('should show agent status legend', async ({ page }) => {
      await page.goto('/agents')

      await expect(page.locator('text=Agent Status')).toBeVisible()
      await expect(page.locator('.glass >> text=Busy')).toBeVisible()
      await expect(page.locator('.glass >> text=Idle')).toBeVisible()
      await expect(page.locator('.glass >> text=Error')).toBeVisible()
      await expect(page.locator('.glass >> text=Paused')).toBeVisible()
    })

    test('should display hexagon agents in grid', async ({ page, wsMock }) => {
      await page.goto('/agents')

      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', status: 'busy' }))

      // Check for SVG hexagons
      const hexagons = page.locator('svg').filter({ has: page.locator('path') })
      await expect(hexagons.first()).toBeVisible({ timeout: 5000 })
    })

    test('should show hover card with agent details', async ({ page, wsMock }) => {
      await page.goto('/agents')

      const agent = createMockAgent({
        id: 'hover-test',
        name: 'HoverAgent',
        model: 'claude-3-opus',
        status: 'busy',
        currentLoad: 7,
        maxLoad: 10,
        successRate: 0.88,
        totalTokens: 250000,
        totalCost: 12.5,
      })
      await wsMock.sendAgentUpdate(agent)

      // Hover over agent
      const agentHex = page.locator('.absolute.cursor-pointer').first()
      await agentHex.hover()

      // Check hover card content
      await expect(page.locator('.glass >> text=HoverAgent')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('.glass >> text=claude-3-opus')).toBeVisible()
      await expect(page.locator('text=/7\\/10/')).toBeVisible()
      await expect(page.locator('text=/88\\.0%/')).toBeVisible()
    })
  })

  test.describe('Agent List View', () => {
    test.beforeEach(async ({ page }) => {
      await page.goto('/agents')
      // Switch to list view
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()
    })

    test('should display agent rows with all columns', async ({ page, wsMock }) => {
      const agent = createMockAgent({
        id: 'list-agent-1',
        name: 'ListAgent',
        model: 'gpt-4-turbo',
        status: 'busy',
        currentLoad: 5,
        maxLoad: 10,
        successRate: 0.95,
        totalCost: 25.5,
      })
      await wsMock.sendAgentUpdate(agent)

      await expect(page.locator('text=ListAgent')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=gpt-4-turbo')).toBeVisible()
      await expect(page.locator('text=/5\\/10/')).toBeVisible()
      await expect(page.locator('text=/95\\.0%/')).toBeVisible()
    })

    test('should show status badge with correct color', async ({ page, wsMock }) => {
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a1', name: 'BusyAgent', status: 'busy' })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a2', name: 'IdleAgent', status: 'idle' })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a3', name: 'ErrorAgent', status: 'error' })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a4', name: 'PausedAgent', status: 'paused' })
      )

      await expect(
        page.locator('span', { hasText: 'busy' }).filter({ hasText: /^busy$/ })
      ).toHaveClass(/text-blue-500/, { timeout: 5000 })
      await expect(
        page.locator('span', { hasText: 'idle' }).filter({ hasText: /^idle$/ })
      ).toHaveClass(/text-gray-400/)
      await expect(
        page.locator('span', { hasText: 'error' }).filter({ hasText: /^error$/ })
      ).toHaveClass(/text-red-500/)
      await expect(
        page.locator('span', { hasText: 'paused' }).filter({ hasText: /^paused$/ })
      ).toHaveClass(/text-yellow-500/)
    })

    test('should show load progress bar', async ({ page, wsMock }) => {
      await wsMock.sendAgentUpdate(
        createMockAgent({
          id: 'load-test',
          name: 'LoadAgent',
          currentLoad: 7,
          maxLoad: 10,
        })
      )

      // Progress bar should show 70% width
      const progressBar = page.locator('.h-full.bg-blue-500.rounded-full').first()
      await expect(progressBar).toHaveAttribute('style', /width:\s*70%/, { timeout: 5000 })
    })

    test('should color-code success rate', async ({ page, wsMock }) => {
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a1', name: 'HighSuccess', successRate: 0.98 })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a2', name: 'MediumSuccess', successRate: 0.85 })
      )
      await wsMock.sendAgentUpdate(
        createMockAgent({ id: 'a3', name: 'LowSuccess', successRate: 0.7 })
      )

      await expect(page.locator('text=/98\\.0%/').first()).toHaveClass(/text-green-500/, {
        timeout: 5000,
      })
      await expect(page.locator('text=/85\\.0%/').first()).toHaveClass(/text-yellow-500/)
      await expect(page.locator('text=/70\\.0%/').first()).toHaveClass(/text-red-500/)
    })

    test('should show action buttons for each agent', async ({ page, wsMock }) => {
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'action-test', name: 'ActionAgent' }))

      // Pause/Play button and More button should be visible
      const row = page.locator('[class*="grid-cols"]').filter({ hasText: 'ActionAgent' })
      await expect(row.locator('button').first()).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Real-time Updates', () => {
    test('should update agent status in real-time', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Switch to list view
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()

      // Send initial agent
      await wsMock.sendAgentUpdate(
        createMockAgent({
          id: 'realtime-agent',
          name: 'RealtimeAgent',
          status: 'idle',
        })
      )

      await expect(
        page.locator('span', { hasText: /^idle$/ }).filter({ hasText: /^idle$/ })
      ).toBeVisible({ timeout: 5000 })

      // Send update
      await wsMock.sendAgentUpdate(
        createMockAgent({
          id: 'realtime-agent',
          name: 'RealtimeAgent',
          status: 'busy',
        })
      )

      await expect(
        page.locator('span', { hasText: /^busy$/ }).filter({ hasText: /^busy$/ })
      ).toBeVisible({ timeout: 5000 })
    })

    test('should add new agents dynamically', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Switch to list view
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()

      // Start with one agent
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a1', name: 'FirstAgent' }))
      await expect(page.locator('text=FirstAgent')).toBeVisible({ timeout: 5000 })

      // Add second agent
      await wsMock.sendAgentUpdate(createMockAgent({ id: 'a2', name: 'SecondAgent' }))
      await expect(page.locator('text=SecondAgent')).toBeVisible({ timeout: 5000 })

      // Both agents should be visible
      await expect(page.locator('text=FirstAgent')).toBeVisible()
    })

    test('should update agent metrics in real-time', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Switch to list view
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()

      // Send initial agent
      await wsMock.sendAgentUpdate(
        createMockAgent({
          id: 'metric-agent',
          name: 'MetricAgent',
          currentLoad: 2,
          maxLoad: 10,
          totalCost: 5.0,
        })
      )

      await expect(page.locator('text=/2\\/10/')).toBeVisible({ timeout: 5000 })

      // Update metrics
      await wsMock.sendAgentUpdate(
        createMockAgent({
          id: 'metric-agent',
          name: 'MetricAgent',
          currentLoad: 8,
          maxLoad: 10,
          totalCost: 15.0,
        })
      )

      await expect(page.locator('text=/8\\/10/')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Animations', () => {
    test('should animate agent entry in list view', async ({ page, wsMock }) => {
      await page.goto('/agents')

      // Switch to list view
      await page.locator('.bg-apex-bg-secondary.rounded-lg.p-1 button').last().click()

      // Send agents
      for (let i = 0; i < 3; i++) {
        await wsMock.sendAgentUpdate(createMockAgent({ id: `anim-${i}`, name: `AnimAgent${i}` }))
      }

      // All agents should eventually be visible
      await expect(page.locator('text=AnimAgent0')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=AnimAgent1')).toBeVisible()
      await expect(page.locator('text=AnimAgent2')).toBeVisible()
    })
  })

  test.describe('Responsive Design', () => {
    test('should adapt to mobile viewport', async ({ page, wsMock }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/agents')

      await wsMock.sendAgentUpdate(createMockAgent({ id: 'mobile-agent' }))

      // Page should still be functional
      await expect(page.locator('h1')).toHaveText('Agents')
      await expect(page.locator('text=/1 agents/')).toBeVisible({ timeout: 5000 })
    })

    test('should adapt filters to tablet viewport', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 })
      await page.goto('/agents')

      // Filters should wrap properly
      await expect(page.locator('input[placeholder="Search agents..."]')).toBeVisible()
      await expect(page.locator('button', { hasText: /^All/ })).toBeVisible()
    })
  })
})
