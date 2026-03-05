import { test, expect, createMockApproval, MockApproval } from './fixtures'

test.describe('Approvals Page', () => {
  test.describe('Page Load and Layout', () => {
    test('should display approvals page header', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('h1')).toHaveText('Approval Queue')
      await expect(
        page.locator('text=Review and approve high-impact agent actions')
      ).toBeVisible()
    })

    test('should display keyboard shortcuts hint', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('text=Keyboard shortcuts:')).toBeVisible()
      await expect(page.locator('kbd', { hasText: 'j' })).toBeVisible()
      await expect(page.locator('kbd', { hasText: 'k' })).toBeVisible()
      await expect(page.locator('kbd', { hasText: 'a' })).toBeVisible()
      await expect(page.locator('kbd', { hasText: 'd' })).toBeVisible()
    })
  })

  test.describe('Stats Cards', () => {
    test('should display all stat cards', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('text=Pending')).toBeVisible()
      await expect(page.locator('text=Approved')).toBeVisible()
      await expect(page.locator('text=Denied')).toBeVisible()
    })

    test('should update stat counts when approvals are added', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Send pending approvals
      await wsMock.sendApprovalRequest(createMockApproval({ id: 'a1', status: 'pending' }))
      await wsMock.sendApprovalRequest(createMockApproval({ id: 'a2', status: 'pending' }))

      // Check pending count
      const pendingCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: /Pending/ })
      await expect(pendingCard.locator('.text-2xl')).toHaveText('2', { timeout: 5000 })
    })

    test('should show correct icons with colors', async ({ page }) => {
      await page.goto('/approvals')

      // Pending should have Clock icon with yellow
      await expect(page.locator('.bg-yellow-500\\/10')).toBeVisible()

      // Approved should have ShieldCheck icon with green
      await expect(page.locator('.bg-green-500\\/10')).toBeVisible()

      // Denied should have ShieldAlert icon with red
      await expect(page.locator('.bg-red-500\\/10')).toBeVisible()
    })
  })

  test.describe('Filter Buttons', () => {
    test('should display filter buttons', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('button', { hasText: 'Pending' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'Resolved' })).toBeVisible()
      await expect(page.locator('button', { hasText: 'All' })).toBeVisible()
    })

    test('should default to pending filter', async ({ page }) => {
      await page.goto('/approvals')

      const pendingButton = page.locator('button', { hasText: 'Pending' }).first()
      await expect(pendingButton).toHaveClass(/bg-apex-accent-primary/)
    })

    test('should filter by pending status', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Send approvals with different statuses
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Pending Action', status: 'pending' })
      )

      // Click pending filter (should be default but click to ensure)
      await page.locator('button', { hasText: 'Pending' }).first().click()

      await expect(page.locator('text=Pending Action')).toBeVisible({ timeout: 5000 })
    })

    test('should filter by resolved status', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Store will directly receive these, not via WebSocket for resolved
      // For testing, we'll use the updateApproval flow

      // First send pending
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Pending Action', status: 'pending' })
      )

      // Wait for it to appear
      await expect(page.locator('text=Pending Action')).toBeVisible({ timeout: 5000 })

      // Approve it via clicking the approve button
      await page.locator('.bg-green-500\\/10').locator('button').first().click()

      // Now switch to resolved filter
      await page.locator('button', { hasText: 'Resolved' }).click()

      // Should see the approved action
      await expect(page.locator('text=Pending Action')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=approved')).toBeVisible()
    })

    test('should show all approvals when All filter selected', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'First Action', status: 'pending' })
      )

      await expect(page.locator('text=First Action')).toBeVisible({ timeout: 5000 })

      // Approve first action
      await page.locator('.bg-green-500\\/10').locator('button').first().click()

      // Send another pending
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a2', actionType: 'Second Action', status: 'pending' })
      )

      // Switch to All filter
      await page.locator('button', { hasText: 'All' }).click()

      // Should see both
      await expect(page.locator('text=First Action')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Second Action')).toBeVisible()
    })

    test('should highlight active filter', async ({ page }) => {
      await page.goto('/approvals')

      // Click Resolved filter
      await page.locator('button', { hasText: 'Resolved' }).click()
      await expect(page.locator('button', { hasText: 'Resolved' })).toHaveClass(
        /bg-apex-accent-primary/
      )

      // Click All filter
      await page.locator('button', { hasText: 'All' }).click()
      await expect(page.locator('button', { hasText: 'All' })).toHaveClass(/bg-apex-accent-primary/)
    })
  })

  test.describe('Approval List Display', () => {
    test('should display approval with action type', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'file_write',
          agentId: 'agent-abc123',
          taskId: 'task-xyz789',
        })
      )

      await expect(page.locator('text=file_write')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=/Agent:.*agent-abc/')).toBeVisible()
      await expect(page.locator('text=/Task:.*task-xyz/')).toBeVisible()
    })

    test('should display risk score badge', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'Test Action',
          riskScore: 0.75,
        })
      )

      await expect(page.locator('text=/Risk:.*75%/')).toBeVisible({ timeout: 5000 })
    })

    test('should color-code risk score - high risk (red)', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'High Risk Action',
          riskScore: 0.9,
        })
      )

      const riskBadge = page.locator('text=/Risk:.*90%/')
      await expect(riskBadge).toHaveClass(/text-red-500/, { timeout: 5000 })
    })

    test('should color-code risk score - medium risk (yellow)', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'Medium Risk Action',
          riskScore: 0.6,
        })
      )

      const riskBadge = page.locator('text=/Risk:.*60%/')
      await expect(riskBadge).toHaveClass(/text-yellow-500/, { timeout: 5000 })
    })

    test('should color-code risk score - low risk (green)', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'Low Risk Action',
          riskScore: 0.3,
        })
      )

      const riskBadge = page.locator('text=/Risk:.*30%/')
      await expect(riskBadge).toHaveClass(/text-green-500/, { timeout: 5000 })
    })

    test('should highlight pending approvals with border', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Pending', status: 'pending' })
      )

      const approvalCard = page.locator('.rounded-lg.border').filter({ hasText: 'Pending' })
      await expect(approvalCard).toHaveClass(/border-yellow-500/, { timeout: 5000 })
    })
  })

  test.describe('Approval Actions', () => {
    test('should show approve and deny buttons for pending approvals', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Test Action', status: 'pending' })
      )

      await expect(page.locator('text=Test Action')).toBeVisible({ timeout: 5000 })

      // Approve button (green background with Check icon)
      await expect(page.locator('.bg-green-500\\/10').locator('button').first()).toBeVisible()

      // Deny button (red background with X icon)
      await expect(page.locator('.bg-red-500\\/10').locator('button').first()).toBeVisible()
    })

    test('should approve approval on click', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'approve-test', actionType: 'Approvable Action', status: 'pending' })
      )

      await expect(page.locator('text=Approvable Action')).toBeVisible({ timeout: 5000 })

      // Click approve button
      const approveButton = page.locator('.bg-green-500\\/10').locator('button').first()
      await approveButton.click()

      // Should show success toast
      await expect(page.locator('text=Request approved')).toBeVisible({ timeout: 5000 })

      // Approval should no longer appear in pending (default filter)
      await expect(page.locator('text=Approvable Action')).not.toBeVisible({ timeout: 5000 })
    })

    test('should deny approval on click', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'deny-test', actionType: 'Deniable Action', status: 'pending' })
      )

      await expect(page.locator('text=Deniable Action')).toBeVisible({ timeout: 5000 })

      // Click deny button
      const denyButton = page.locator('.bg-red-500\\/10').locator('button').first()
      await denyButton.click()

      // Should show success toast
      await expect(page.locator('text=Request denied')).toBeVisible({ timeout: 5000 })

      // Approval should no longer appear in pending
      await expect(page.locator('text=Deniable Action')).not.toBeVisible({ timeout: 5000 })
    })

    test('should not show action buttons for resolved approvals', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Send pending and approve it
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Resolved Action', status: 'pending' })
      )

      await expect(page.locator('text=Resolved Action')).toBeVisible({ timeout: 5000 })

      // Approve it
      await page.locator('.bg-green-500\\/10').locator('button').first().click()

      // Switch to resolved filter
      await page.locator('button', { hasText: 'Resolved' }).click()

      // Should show status badge instead of buttons
      await expect(page.locator('text=Resolved Action')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('span', { hasText: 'approved' })).toBeVisible()
    })
  })

  test.describe('Bulk Actions', () => {
    test('should show Approve All button when pending approvals exist', async ({
      page,
      wsMock,
    }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(createMockApproval({ id: 'a1', status: 'pending' }))
      await wsMock.sendApprovalRequest(createMockApproval({ id: 'a2', status: 'pending' }))

      await expect(page.locator('button', { hasText: /Approve All \(2\)/ })).toBeVisible({
        timeout: 5000,
      })
    })

    test('should not show Approve All button when no pending approvals', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('button', { hasText: /Approve All/ })).not.toBeVisible()
    })

    test('should approve all pending approvals on bulk approve', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Action 1', status: 'pending' })
      )
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a2', actionType: 'Action 2', status: 'pending' })
      )

      await expect(page.locator('text=Action 1')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=Action 2')).toBeVisible()

      // Click Approve All
      await page.locator('button', { hasText: /Approve All/ }).click()

      // Should show success toast
      await expect(page.locator('text=/Approved 2 requests/')).toBeVisible({ timeout: 5000 })

      // No pending approvals should remain
      await expect(page.locator('text=No pending approvals')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Approval Expansion', () => {
    test('should expand approval to show action data', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'expand-test',
          actionType: 'Expandable Action',
          actionData: { path: '/tmp/test.txt', content: 'Test content' },
          status: 'pending',
        })
      )

      await expect(page.locator('text=Expandable Action')).toBeVisible({ timeout: 5000 })

      // Click to expand
      await page.locator('.cursor-pointer').filter({ hasText: 'Expandable Action' }).click()

      // Should show Action Data section
      await expect(page.locator('text=Action Data')).toBeVisible({ timeout: 5000 })
      await expect(page.locator('text=/\\/tmp\\/test\\.txt/')).toBeVisible()
      await expect(page.locator('text=/Test content/')).toBeVisible()
    })

    test('should show creation date in expanded view', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'Date Action',
          status: 'pending',
        })
      )

      // Expand
      await page.locator('.cursor-pointer').filter({ hasText: 'Date Action' }).click()

      await expect(page.locator('text=Created:')).toBeVisible({ timeout: 5000 })
    })

    test('should collapse on second click', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'Toggle Action',
          status: 'pending',
        })
      )

      // Expand
      await page.locator('.cursor-pointer').filter({ hasText: 'Toggle Action' }).click()
      await expect(page.locator('text=Action Data')).toBeVisible({ timeout: 5000 })

      // Collapse
      await page.locator('.cursor-pointer').filter({ hasText: 'Toggle Action' }).click()
      await expect(page.locator('text=Action Data')).not.toBeVisible({ timeout: 5000 })
    })

    test('should format JSON action data nicely', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({
          id: 'a1',
          actionType: 'JSON Action',
          actionData: { key: 'value', nested: { inner: true } },
          status: 'pending',
        })
      )

      // Expand
      await page.locator('.cursor-pointer').filter({ hasText: 'JSON Action' }).click()

      // Should have pre tag for formatted JSON
      const preBlock = page.locator('pre')
      await expect(preBlock).toBeVisible({ timeout: 5000 })
      await expect(preBlock).toContainText('key')
      await expect(preBlock).toContainText('value')
    })
  })

  test.describe('Real-time Updates', () => {
    test('should receive new approval requests via WebSocket', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Initially empty
      await expect(page.locator('text=No pending approvals')).toBeVisible()

      // Send approval request
      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'realtime-1', actionType: 'Realtime Action', status: 'pending' })
      )

      // Should appear immediately
      await expect(page.locator('text=Realtime Action')).toBeVisible({ timeout: 5000 })

      // Pending count should update
      const pendingCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: /Pending/ })
      await expect(pendingCard.locator('.text-2xl')).toHaveText('1')
    })

    test('should update stats when approvals are processed', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Send 3 pending approvals
      for (let i = 0; i < 3; i++) {
        await wsMock.sendApprovalRequest(
          createMockApproval({ id: `a${i}`, actionType: `Action ${i}`, status: 'pending' })
        )
      }

      // Pending count should be 3
      const pendingCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: /Pending/ })
      await expect(pendingCard.locator('.text-2xl')).toHaveText('3', { timeout: 5000 })

      // Approve one
      await page.locator('.bg-green-500\\/10').locator('button').first().click()

      // Pending count should be 2, Approved should be 1
      await expect(pendingCard.locator('.text-2xl')).toHaveText('2', { timeout: 5000 })
      const approvedCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: /Approved/ })
      await expect(approvedCard.locator('.text-2xl')).toHaveText('1')
    })

    test('should handle rapid approval submissions', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      // Send multiple approvals rapidly
      const approvals: MockApproval[] = []
      for (let i = 0; i < 5; i++) {
        approvals.push(createMockApproval({ id: `rapid-${i}`, actionType: `Rapid ${i}`, status: 'pending' }))
      }

      for (const approval of approvals) {
        await wsMock.sendApprovalRequest(approval)
      }

      // All should appear
      const pendingCard = page.locator('.bg-apex-bg-secondary').filter({ hasText: /Pending/ })
      await expect(pendingCard.locator('.text-2xl')).toHaveText('5', { timeout: 5000 })
    })
  })

  test.describe('Empty States', () => {
    test('should show "No pending approvals" when empty', async ({ page }) => {
      await page.goto('/approvals')

      await expect(page.locator('text=No pending approvals')).toBeVisible()
    })

    test('should show "No approvals found" for resolved filter when empty', async ({ page }) => {
      await page.goto('/approvals')

      await page.locator('button', { hasText: 'Resolved' }).click()

      await expect(page.locator('text=No approvals found')).toBeVisible()
    })
  })

  test.describe('Toast Notifications', () => {
    test('should show success toast on approve', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Toast Test', status: 'pending' })
      )

      await page.locator('.bg-green-500\\/10').locator('button').first().click()

      await expect(page.locator('text=Request approved')).toBeVisible({ timeout: 5000 })
    })

    test('should show success toast on deny', async ({ page, wsMock }) => {
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Toast Test', status: 'pending' })
      )

      await page.locator('.bg-red-500\\/10').locator('button').first().click()

      await expect(page.locator('text=Request denied')).toBeVisible({ timeout: 5000 })
    })
  })

  test.describe('Responsive Design', () => {
    test('should adapt to mobile viewport', async ({ page, wsMock }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/approvals')

      await wsMock.sendApprovalRequest(
        createMockApproval({ id: 'a1', actionType: 'Mobile Action', status: 'pending' })
      )

      await expect(page.locator('h1')).toHaveText('Approval Queue')
      await expect(page.locator('text=Mobile Action')).toBeVisible({ timeout: 5000 })
    })

    test('should stack stat cards on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 })
      await page.goto('/approvals')

      // All stat cards should still be visible
      await expect(page.locator('text=Pending')).toBeVisible()
      await expect(page.locator('text=Approved')).toBeVisible()
      await expect(page.locator('text=Denied')).toBeVisible()
    })
  })
})
