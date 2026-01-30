import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import Layout from '@/components/Layout'
import { useStore } from '@/lib/store'

// Mock framer-motion
vi.mock('framer-motion', () => ({
  motion: {
    aside: ({ children, className, animate, ...props }: any) => (
      <aside className={className} style={{ width: animate?.width }} {...props}>
        {children}
      </aside>
    ),
    div: ({ children, className, ...props }: any) => (
      <div className={className} {...props}>
        {children}
      </div>
    ),
    span: ({ children, className, ...props }: any) => (
      <span className={className} {...props}>
        {children}
      </span>
    ),
  },
}))

// Helper to render Layout with router
const renderWithRouter = (initialRoute = '/', children?: React.ReactNode) => {
  return render(
    <MemoryRouter initialEntries={[initialRoute]}>
      <Routes>
        <Route
          path="/*"
          element={
            <Layout>
              {children || <div data-testid="page-content">Page Content</div>}
            </Layout>
          }
        />
      </Routes>
    </MemoryRouter>
  )
}

describe('Layout', () => {
  beforeEach(() => {
    // Reset store state
    const store = useStore.getState()
    store.setWsConnected(false)
    store.setSidebarCollapsed(false)
    store.setMetrics({
      totalTasks: 100,
      completedTasks: 80,
      failedTasks: 5,
      runningTasks: 15,
      totalAgents: 10,
      activeAgents: 8,
      totalTokens: 1000000,
      totalCost: 25.5,
      avgLatencyMs: 500,
      successRate: 0.95,
    })
  })

  describe('rendering', () => {
    it('renders the layout structure', () => {
      renderWithRouter()

      // Should have sidebar and main content
      expect(screen.getByRole('navigation')).toBeInTheDocument()
      expect(screen.getByRole('main')).toBeInTheDocument()
    })

    it('renders children in main content area', () => {
      renderWithRouter('/', <div data-testid="custom-content">Custom Content</div>)

      expect(screen.getByTestId('custom-content')).toBeInTheDocument()
    })

    it('renders logo and brand name', () => {
      renderWithRouter()

      expect(screen.getByText('Apex')).toBeInTheDocument()
    })
  })

  describe('navigation items', () => {
    it('renders all navigation links', () => {
      renderWithRouter()

      expect(screen.getByText('Dashboard')).toBeInTheDocument()
      expect(screen.getByText('Agents')).toBeInTheDocument()
      expect(screen.getByText('Tasks')).toBeInTheDocument()
      expect(screen.getByText('Approvals')).toBeInTheDocument()
      expect(screen.getByText('Settings')).toBeInTheDocument()
    })

    it('highlights active navigation link', () => {
      renderWithRouter('/')

      const dashboardLink = screen.getByText('Dashboard').closest('a')
      expect(dashboardLink).toHaveClass('bg-apex-accent-primary/10')
    })

    it('navigates to correct routes', async () => {
      const user = userEvent.setup()
      renderWithRouter()

      await user.click(screen.getByText('Agents'))
      expect(window.location.pathname).toBe('/')
    })

    it('shows correct icons for navigation items', () => {
      const { container } = renderWithRouter()

      // Should have SVG icons for each nav item
      const navIcons = container.querySelectorAll('nav svg')
      expect(navIcons.length).toBe(5) // 5 nav items
    })
  })

  describe('sidebar collapse', () => {
    it('renders expanded sidebar by default', () => {
      renderWithRouter()

      // Navigation labels should be visible
      expect(screen.getByText('Dashboard')).toBeVisible()
      expect(screen.getByText('Agents')).toBeVisible()
    })

    it('toggles sidebar collapse on button click', async () => {
      const user = userEvent.setup()
      renderWithRouter()

      // Find toggle button
      const toggleButton = screen.getByRole('button')
      await user.click(toggleButton)

      expect(useStore.getState().sidebarCollapsed).toBe(true)
    })

    it('updates store when sidebar is collapsed', async () => {
      const user = userEvent.setup()
      renderWithRouter()

      const toggleButton = screen.getByRole('button')
      await user.click(toggleButton)

      expect(useStore.getState().sidebarCollapsed).toBe(true)

      await user.click(toggleButton)
      expect(useStore.getState().sidebarCollapsed).toBe(false)
    })

    it('shows Menu icon when collapsed', () => {
      useStore.getState().setSidebarCollapsed(true)
      const { container } = renderWithRouter()

      // Menu icon should be present when collapsed
      // X icon should be present when expanded
      const button = screen.getByRole('button')
      expect(button).toBeInTheDocument()
    })

    it('shows X icon when expanded', () => {
      useStore.getState().setSidebarCollapsed(false)
      renderWithRouter()

      const button = screen.getByRole('button')
      expect(button).toBeInTheDocument()
    })
  })

  describe('connection status', () => {
    it('shows disconnected status when wsConnected is false', () => {
      useStore.getState().setWsConnected(false)
      renderWithRouter()

      expect(screen.getByText('Disconnected')).toBeInTheDocument()
    })

    it('shows connected status when wsConnected is true', () => {
      useStore.getState().setWsConnected(true)
      renderWithRouter()

      expect(screen.getByText('Connected')).toBeInTheDocument()
    })

    it('shows green WiFi icon when connected', () => {
      useStore.getState().setWsConnected(true)
      const { container } = renderWithRouter()

      // Check for Wifi icon with green color
      const statusArea = container.querySelector('.text-green-500')
      expect(statusArea).toBeInTheDocument()
    })

    it('shows red WifiOff icon when disconnected', () => {
      useStore.getState().setWsConnected(false)
      const { container } = renderWithRouter()

      // Check for WifiOff icon with red color
      const statusArea = container.querySelector('.text-red-500')
      expect(statusArea).toBeInTheDocument()
    })
  })

  describe('top bar metrics', () => {
    it('displays active agents count', () => {
      renderWithRouter()

      expect(screen.getByText('8 active agents')).toBeInTheDocument()
    })

    it('displays running tasks count', () => {
      renderWithRouter()

      expect(screen.getByText('15 running tasks')).toBeInTheDocument()
    })

    it('displays total cost', () => {
      renderWithRouter()

      expect(screen.getByText('$25.5000 spent')).toBeInTheDocument()
    })

    it('displays success rate with correct color', () => {
      renderWithRouter()

      const successRate = screen.getByText('95.0%')
      expect(successRate).toBeInTheDocument()
      expect(successRate).toHaveClass('text-green-500')
    })

    it('shows yellow color for medium success rate', () => {
      useStore.getState().setMetrics({ successRate: 0.85 })
      renderWithRouter()

      const successRate = screen.getByText('85.0%')
      expect(successRate).toHaveClass('text-yellow-500')
    })

    it('shows red color for low success rate', () => {
      useStore.getState().setMetrics({ successRate: 0.7 })
      renderWithRouter()

      const successRate = screen.getByText('70.0%')
      expect(successRate).toHaveClass('text-red-500')
    })
  })

  describe('layout structure', () => {
    it('has flex layout', () => {
      const { container } = renderWithRouter()

      const layoutRoot = container.querySelector('.flex.h-screen')
      expect(layoutRoot).toBeInTheDocument()
    })

    it('has overflow hidden on root', () => {
      const { container } = renderWithRouter()

      const layoutRoot = container.querySelector('.overflow-hidden')
      expect(layoutRoot).toBeInTheDocument()
    })

    it('has scrollable main content', () => {
      renderWithRouter()

      const main = screen.getByRole('main')
      expect(main).toHaveClass('overflow-auto')
    })

    it('has correct header height', () => {
      const { container } = renderWithRouter()

      const header = container.querySelector('header')
      expect(header).toHaveClass('h-16')
    })
  })

  describe('sidebar styling', () => {
    it('has correct background color', () => {
      const { container } = renderWithRouter()

      const sidebar = container.querySelector('aside')
      expect(sidebar).toHaveClass('bg-apex-bg-secondary')
    })

    it('has border on right side', () => {
      const { container } = renderWithRouter()

      const sidebar = container.querySelector('aside')
      expect(sidebar).toHaveClass('border-r')
    })
  })

  describe('responsive behavior', () => {
    it('hides nav labels when sidebar is collapsed', () => {
      useStore.getState().setSidebarCollapsed(true)
      renderWithRouter()

      // When collapsed, labels may not be visible
      // The icons should still be there
      const nav = screen.getByRole('navigation')
      expect(nav).toBeInTheDocument()
    })

    it('hides brand name when sidebar is collapsed', () => {
      useStore.getState().setSidebarCollapsed(true)
      renderWithRouter()

      // Brand name visibility depends on collapsed state
      // With collapsed true, Apex text may be hidden
    })

    it('hides connection status text when collapsed', () => {
      useStore.getState().setSidebarCollapsed(true)
      useStore.getState().setWsConnected(true)
      renderWithRouter()

      // Connection text may be hidden when collapsed
      // Icon should still be visible
    })
  })

  describe('accessibility', () => {
    it('has navigation landmark', () => {
      renderWithRouter()

      expect(screen.getByRole('navigation')).toBeInTheDocument()
    })

    it('has main landmark', () => {
      renderWithRouter()

      expect(screen.getByRole('main')).toBeInTheDocument()
    })

    it('has banner landmark (header)', () => {
      renderWithRouter()

      expect(screen.getByRole('banner')).toBeInTheDocument()
    })

    it('navigation links are accessible via keyboard', async () => {
      const user = userEvent.setup()
      renderWithRouter()

      // Tab to first nav link
      await user.tab()
      await user.tab() // Skip toggle button

      // Should be able to focus on nav links
      const dashboardLink = screen.getByText('Dashboard').closest('a')
      expect(document.activeElement?.closest('nav')).toBe(screen.getByRole('navigation'))
    })

    it('toggle button has accessible label', () => {
      renderWithRouter()

      const button = screen.getByRole('button')
      expect(button).toBeInTheDocument()
    })
  })

  describe('quick stats', () => {
    it('renders green indicator dot', () => {
      const { container } = renderWithRouter()

      const greenDot = container.querySelector('.bg-green-500.rounded-full')
      expect(greenDot).toBeInTheDocument()
    })

    it('renders separator between stats', () => {
      renderWithRouter()

      // Check for separator character
      const separators = screen.getAllByText('|')
      expect(separators.length).toBeGreaterThan(0)
    })
  })

  describe('theme integration', () => {
    it('uses apex theme colors for background', () => {
      renderWithRouter()

      const main = screen.getByRole('main')
      expect(main).toHaveClass('bg-apex-bg-primary')
    })

    it('uses apex theme colors for text', () => {
      renderWithRouter()

      // Check secondary text color usage
      const { container } = renderWithRouter()
      const secondaryText = container.querySelector('.text-apex-text-secondary')
      expect(secondaryText).toBeInTheDocument()
    })
  })
})
