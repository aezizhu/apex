import { Page, Locator } from '@playwright/test'

/**
 * Utility functions for E2E tests.
 */

/**
 * Wait for a specific number of elements to be visible.
 */
export async function waitForElementCount(
  locator: Locator,
  count: number,
  timeout = 5000
): Promise<void> {
  await locator.first().waitFor({ state: 'visible', timeout })
  let currentCount = await locator.count()
  const startTime = Date.now()

  while (currentCount !== count && Date.now() - startTime < timeout) {
    await new Promise((resolve) => setTimeout(resolve, 100))
    currentCount = await locator.count()
  }

  if (currentCount !== count) {
    throw new Error(`Expected ${count} elements, but found ${currentCount}`)
  }
}

/**
 * Wait for WebSocket to be connected (by checking for live indicator).
 */
export async function waitForWebSocketConnection(page: Page, timeout = 10000): Promise<void> {
  await page.locator('text=Live').waitFor({ state: 'visible', timeout })
}

/**
 * Navigate to a page and wait for it to be fully loaded.
 */
export async function navigateAndWait(page: Page, path: string): Promise<void> {
  await page.goto(path)
  await page.waitForLoadState('networkidle')
}

/**
 * Get the text content of a stat card by label.
 */
export async function getStatCardValue(page: Page, label: string): Promise<string> {
  const card = page.locator('.bg-apex-bg-secondary').filter({ hasText: label })
  const value = await card.locator('.text-2xl').textContent()
  return value || ''
}

/**
 * Wait for toast notification to appear.
 */
export async function waitForToast(page: Page, text: string | RegExp, timeout = 5000): Promise<void> {
  const toastLocator =
    typeof text === 'string' ? page.locator(`text=${text}`) : page.locator(`text=${text}`)
  await toastLocator.waitFor({ state: 'visible', timeout })
}

/**
 * Dismiss all visible toasts.
 */
export async function dismissToasts(page: Page): Promise<void> {
  const toasts = page.locator('[role="status"]')
  const count = await toasts.count()

  for (let i = 0; i < count; i++) {
    try {
      await toasts.nth(i).click()
    } catch {
      // Toast may have auto-dismissed
    }
  }
}

/**
 * Simulate typing with realistic delays.
 */
export async function typeWithDelay(
  locator: Locator,
  text: string,
  delay = 50
): Promise<void> {
  await locator.pressSequentially(text, { delay })
}

/**
 * Clear and type new text in an input.
 */
export async function clearAndType(
  locator: Locator,
  text: string
): Promise<void> {
  await locator.clear()
  await locator.fill(text)
}

/**
 * Check if an element has a specific CSS class.
 */
export async function hasClass(locator: Locator, className: string): Promise<boolean> {
  const classes = await locator.getAttribute('class')
  return classes?.includes(className) || false
}

/**
 * Wait for animation to complete.
 */
export async function waitForAnimation(page: Page, duration = 300): Promise<void> {
  await page.waitForTimeout(duration)
}

/**
 * Take a screenshot with timestamp.
 */
export async function screenshotWithTimestamp(
  page: Page,
  name: string
): Promise<Buffer> {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
  return page.screenshot({
    path: `playwright-report/screenshots/${name}-${timestamp}.png`,
    fullPage: true,
  })
}

/**
 * Get all visible text from a locator.
 */
export async function getAllText(locator: Locator): Promise<string[]> {
  return locator.allTextContents()
}

/**
 * Check if page has no console errors.
 */
export async function checkNoConsoleErrors(page: Page): Promise<string[]> {
  const errors: string[] = []

  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      errors.push(msg.text())
    }
  })

  return errors
}

/**
 * Retry an action with exponential backoff.
 */
export async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxRetries = 3,
  initialDelay = 100
): Promise<T> {
  let lastError: Error | undefined

  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn()
    } catch (error) {
      lastError = error as Error
      await new Promise((resolve) => setTimeout(resolve, initialDelay * Math.pow(2, i)))
    }
  }

  throw lastError
}

/**
 * Scroll element into view.
 */
export async function scrollIntoView(locator: Locator): Promise<void> {
  await locator.scrollIntoViewIfNeeded()
}

/**
 * Get computed style property.
 */
export async function getComputedStyle(
  page: Page,
  selector: string,
  property: string
): Promise<string> {
  return page.evaluate(
    ([sel, prop]) => {
      const element = document.querySelector(sel)
      if (!element) return ''
      return window.getComputedStyle(element).getPropertyValue(prop)
    },
    [selector, property]
  )
}
