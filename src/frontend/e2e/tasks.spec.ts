import { test, expect, createMockTask, MockTask } from './fixtures'

test.describe('Tasks Page', () => {
  test.describe('Page Load and Layout', () => {
    test('should display tasks page header', async ({ page }) => {
      await page.goto('/tasks')

      await expect(page.locator('h1')).toHaveText('Tasks')
      await expect(page.locator('text=Monitor and manage task execution')).toBeVisible()
    })

    test('should display Submit Task button', async ({ page }) => {
      await page.goto('/tasks')

      const submitButton = page.locator('button', { hasText: 'Submit Task' })
      await expect(submitButton).toBeVisible()
    })

    test('should display search input', async ({ page }) => {
      await page.goto('/tasks')

      const searchInput = page.locator('input[placeholder="Search tasks..."]')
      await expect(searchInput).toBeVisible()
    })

    test('should display status dropdown filter', async ({ page }) => {
      await page.goto('/tasks')

      const statusSelect = page.locator('select')
      await expect(statusSelect).toBeVisible()
      await expect(statusSelect).toHaveValue('all')
    })
  })

  test.describe('Stats Cards', () => {
    test('should display all stat cards', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      // Send tasks with different statuses
      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', status: 'pending' }))
      await wsMock.sendTaskUpdate(createMockTask({ id: 't2', status: 'running' }))
      await wsMock.sendTaskUpdate(createMockTask({ id: 't3', status: 'completed' }))
      await wsMock.sendTaskUpdate(createMockTask({ id: 't4', status: 'failed' }))

      // Check stat cards
      await expect(page.locator('text=Pending')).toBeVisible()
      await expect(page.locator('text=Running')).toBeVisible()
      await expect(page.locator('text=Completed')).toBeVisible()
      await expect(page.locator('text=Failed')).toBeVisible()
    })

    test('should update stat counts in real-time', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      // Send initial tasks
      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', status: 'running' }))
      await wsMock.sendTaskUpdate(createMockTask({ id: 't2', status: 'running' }))

      // Find the Running stat card and check value
      const runningCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Running' })
      await expect(runningCard.locator('.text-2xl')).toHaveText('2', { timeout: 5000 })

      // Update task status
      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', status: 'completed' }))

      // Verify counts updated
      await expect(runningCard.locator('.text-2xl')).toHaveText('1', { timeout: 5000 })
    })

    test('should show correct color for each stat', async ({ page }) => {
      await page.goto('/tasks')

      await expect(page.locator('.text-yellow-500').filter({ hasText: 'Pending' })).toBeVisible()
      await expect(page.locator('.text-blue-500').filter({ hasText: 'Running' })).toBeVisible()
      await expect(page.locator('.text-green-500').filter({ hasText: 'Completed' })).toBeVisible()
      await expect(page.locator('.text-red-500').filter({ hasText: 'Failed' })).toBeVisible()
    })
  })

  test.describe('Status Filtering', () => {
    test.beforeEach(async ({ page, wsMock }) => {
      await page.goto('/tasks')

      // Create tasks with different statuses
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Pending Task', status: 'pending' })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't2', name: 'Running Task', status: 'running' })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't3', name: 'Completed Task', status: 'completed' })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't4', name: 'Failed Task', status: 'failed' })
      )
    })

    test('should show all tasks by default', async ({ page }) => {
      await expect(page.locator('text=Pending Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Running Task')).toBeVisible()
      await expect(page.locator('text=Completed Task')).toBeVisible()
      await expect(page.locator('text=Failed Task')).toBeVisible()
    })

    test('should filter by pending status', async ({ page }) => {
      await page.selectOption('select', 'pending')

      await expect(page.locator('text=Pending Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Running Task')).not.toBeVisible()
      await expect(page.locator('text=Completed Task')).not.toBeVisible()
      await expect(page.locator('text=Failed Task')).not.toBeVisible()
    })

    test('should filter by running status', async ({ page }) => {
      await page.selectOption('select', 'running')

      await expect(page.locator('text=Running Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Pending Task')).not.toBeVisible()
    })

    test('should filter by completed status', async ({ page }) => {
      await page.selectOption('select', 'completed')

      await expect(page.locator('text=Completed Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Running Task')).not.toBeVisible()
    })

    test('should filter by failed status', async ({ page }) => {
      await page.selectOption('select', 'failed')

      await expect(page.locator('text=Failed Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Completed Task')).not.toBeVisible()
    })

    test('should reset filter to all', async ({ page }) => {
      // First filter
      await page.selectOption('select', 'pending')
      await expect(page.locator('text=Pending Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Running Task')).not.toBeVisible()

      // Reset to all
      await page.selectOption('select', 'all')
      await expect(page.locator('text=Running Task')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Search Functionality', () => {
    test.beforeEach(async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Data Processing Task' })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't2', name: 'API Integration' })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't3', name: 'Report Generation' })
      )
    })

    test('should filter tasks by search query', async ({ page }) => {
      await page.fill('input[placeholder="Search tasks..."]', 'Data')

      await expect(page.locator('text=Data Processing Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=API Integration')).not.toBeVisible()
      await expect(page.locator('text=Report Generation')).not.toBeVisible()
    })

    test('should be case-insensitive', async ({ page }) => {
      await page.fill('input[placeholder="Search tasks..."]', 'data processing')

      await expect(page.locator('text=Data Processing Task')).toBeVisible({ timeout: 5000 })
    })

    test('should show "No tasks found" when no match', async ({ page }) => {
      await page.fill('input[placeholder="Search tasks..."]', 'NonexistentTask')

      await expect(page.locator('text=No tasks found')).toBeVisible({ timeout: 5000 })
    })

    test('should combine search with status filter', async ({ page, wsMock }) => {
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't4', name: 'Data Export', status: 'running' })
      )

      // Filter by running
      await page.selectOption('select', 'running')

      // Search for "Data"
      await page.fill('input[placeholder="Search tasks..."]', 'Data')

      // Should show only running task matching "Data"
      await expect(page.locator('text=Data Export')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Data Processing Task')).not.toBeVisible()
    })

    test('should clear search filter', async ({ page }) => {
      await page.fill('input[placeholder="Search tasks..."]', 'Data')
      await expect(page.locator('text=Data Processing Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=API Integration')).not.toBeVisible()

      // Clear search
      await page.fill('input[placeholder="Search tasks..."]', '')
      await expect(page.locator('text=API Integration')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Task List Display', () => {
    test('should display task with status icon', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Test Task', status: 'completed' })
      )

      // Task should be visible with status icon (CheckCircle for completed)
      await expect(page.locator('text=Test Task')).toBeVisible({ timeout: 5000 })
      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Test Task' })
      await expect(taskRow.locator('svg').first()).toBeVisible()
    })

    test('should display task ID and creation date', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      const taskId = 'task-abc12345'
      await wsMock.sendTaskUpdate(
        createMockTask({ id: taskId, name: 'Test Task' })
      )

      await expect(page.locator('text=Test Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=/task-abc/')).toBeVisible()
    })

    test('should display tokens and cost', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 't1',
          name: 'Costly Task',
          tokensUsed: 5000,
          costDollars: 0.25,
        })
      )

      await expect(page.locator('text=Costly Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=/5,?000.*tokens/')).toBeVisible()
      await expect(page.locator('text=/\\$0\\.25/')).toBeVisible()
    })

    test('should display duration for completed tasks', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      const startTime = new Date(Date.now() - 60000) // 1 minute ago
      const endTime = new Date()

      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 't1',
          name: 'Completed Task',
          status: 'completed',
          startedAt: startTime.toISOString(),
          completedAt: endTime.toISOString(),
        })
      )

      await expect(page.locator('text=Completed Task')).toBeVisible({ timeout: 5000 })
      // Duration should be around 1 minute
      await expect(page.locator('text=/1m|60s|1:00/')).toBeVisible()
    })

    test('should sort tasks by creation date (newest first)', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      const now = Date.now()
      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 't1',
          name: 'Older Task',
          createdAt: new Date(now - 10000).toISOString(),
        })
      )
      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 't2',
          name: 'Newer Task',
          createdAt: new Date(now).toISOString(),
        })
      )

      await expect(page.locator('text=Newer Task')).toBeVisible({ timeout: 5000 })

      // Verify order (Newer Task should appear first)
      const taskList = page.locator('.space-y-2 > div')
      const firstTask = taskList.first()
      await expect(firstTask).toContainText('Newer Task')
    })
  })

  test.describe('Task Expansion', () => {
    test('should expand task to show details', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 'expand-test',
          dagId: 'dag-12345',
          name: 'Expandable Task',
          agentId: 'agent-xyz',
          startedAt: new Date().toISOString(),
        })
      )

      await expect(page.locator('text=Expandable Task')).toBeVisible({ timeout: 5000 })

      // Click to expand
      await page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Expandable Task' }).click()

      // Expanded details should be visible
      await expect(page.locator('text=DAG ID')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=/dag-12345/')).toBeVisible()
      await expect(page.locator('text=Agent')).toBeVisible()
      await expect(page.locator('text=/agent-xyz/')).toBeVisible()
    })

    test('should collapse task on second click', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', dagId: 'dag-test', name: 'Toggle Task' })
      )

      // Expand
      await page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Toggle Task' }).click()
      await expect(page.locator('text=DAG ID')).toBeVisible({ timeout: 5000 })

      // Collapse
      await page.locator('.cursor-pointer').filter({ hasText: 'Toggle Task' }).click()
      await expect(page.locator('text=DAG ID')).not.toBeVisible({ timeout: 5000 })
    })

    test('should show action buttons in expanded view', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Action Task', status: 'completed' })
      )

      // Expand
      await page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Action Task' }).click()

      // Check buttons
      await expect(page.locator('button', { hasText: 'View Details' })).toBeVisible({ timeout: 5000 })
      await expect(page.locator('button', { hasText: 'View Logs' })).toBeVisible()
    })

    test('should show Cancel button for running tasks', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Running Task', status: 'running' })
      )

      // Expand
      await page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Running Task' }).click()

      await expect(page.locator('button', { hasText: 'Cancel' })).toBeVisible({ timeout: 5000 })
    })

    test('should show Retry button for failed tasks', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Failed Task', status: 'failed' })
      )

      // Expand
      await page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Failed Task' }).click()

      await expect(page.locator('button', { hasText: 'Retry' })).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Real-time Updates', () => {
    test('should update task status in real-time', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      // Create initial task
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 'realtime-1', name: 'Realtime Task', status: 'pending' })
      )

      await expect(page.locator('text=Realtime Task')).toBeVisible({ timeout: 5000 })

      // Status icon should be for pending (Clock)
      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Realtime Task' })
      await expect(taskRow.locator('.text-yellow-500').first()).toBeVisible()

      // Update to running
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 'realtime-1', name: 'Realtime Task', status: 'running' })
      )

      // Status icon should change to running (PlayCircle - blue)
      await expect(taskRow.locator('.text-blue-500').first()).toBeVisible({ timeout: 5000 })

      // Update to completed
      await wsMock.sendTaskUpdate(
        createMockTask({ id: 'realtime-1', name: 'Realtime Task', status: 'completed' })
      )

      // Status icon should change to completed (CheckCircle - green)
      await expect(taskRow.locator('.text-green-500').first()).toBeVisible({ timeout: 5000 })
    })

    test('should add new tasks dynamically', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', name: 'First Task' }))
      await expect(page.locator('text=First Task')).toBeVisible({ timeout: 5000 })

      await wsMock.sendTaskUpdate(createMockTask({ id: 't2', name: 'Second Task' }))
      await expect(page.locator('text=Second Task')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=First Task')).toBeVisible()
    })

    test('should update task metrics in real-time', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 'metric-task',
          name: 'Metric Task',
          tokensUsed: 1000,
          costDollars: 0.05,
        })
      )

      await expect(page.locator('text=/1,?000.*tokens/')).toBeVisible({ timeout: 5000 })

      await wsMock.sendTaskUpdate(
        createMockTask({
          id: 'metric-task',
          name: 'Metric Task',
          tokensUsed: 5000,
          costDollars: 0.25,
        })
      )

      await expect(page.locator('text=/5,?000.*tokens/')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Empty State', () => {
    test('should show "No tasks found" when no tasks exist', async ({ page }) => {
      await page.goto('/tasks')

      // Without sending any tasks, should show empty message
      await expect(page.locator('text=No tasks found')).toBeVisible({ timeout: 5000 })
    })

    test('should show empty state when filter yields no results', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      // Add only pending tasks
      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', status: 'pending' }))

      // Filter by failed
      await page.selectOption('select', 'failed')

      await expect(page.locator('text=No tasks found')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Status Icons', () => {
    test('should display correct icon for completed status', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Completed', status: 'completed' })
      )

      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Completed' })
      await expect(taskRow.locator('.text-green-500').first()).toBeVisible({ timeout: 5000 })
    })

    test('should display correct icon for failed status', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', name: 'Failed', status: 'failed' }))

      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: /^Failed/ })
      await expect(taskRow.locator('.text-red-500').first()).toBeVisible({ timeout: 5000 })
    })

    test('should display correct icon for running status', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Running', status: 'running' })
      )

      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Running' }).first()
      await expect(taskRow.locator('.text-blue-500').first()).toBeVisible({ timeout: 5000 })
    })

    test('should display correct icon for pending status', async ({ page, wsMock }) => {
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(
        createMockTask({ id: 't1', name: 'Pending', status: 'pending' })
      )

      const taskRow = page.locator('.bg-apex-bg-secondary').filter({ hasText: 'Pending' }).first()
      await expect(taskRow.locator('.text-yellow-500').first()).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Responsive Design', () => {
    test('should adapt to mobile viewport', async ({ page, wsMock }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/tasks')

      await wsMock.sendTaskUpdate(createMockTask({ id: 't1', name: 'Mobile Task' }))

      await expect(page.locator('h1')).toHaveText('Tasks')
      await expect(page.locator('text=Mobile Task')).toBeVisible({ timeout: 5000 })
    })

    test('should stack stat cards on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/tasks')

      // Stat cards should be visible
      await expect(page.locator('text=Pending')).toBeVisible()
      await expect(page.locator('text=Running')).toBeVisible()
    })
  })
})
