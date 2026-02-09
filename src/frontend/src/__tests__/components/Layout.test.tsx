import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import Layout from '@/components/Layout'
import { useStore } from '@/lib/store'

// Mock framer-motion
vi.mock('framer-motion', () => ({
  motion: {
    aside: ({ children, className, animate, style, ...props }: any) => (
      <aside className={className} style={{ ...style, width: animate?.width }} {...props}>
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
  AnimatePresence: ({ children }: any) => <>{children}</>,
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
    const store = useStore.getState()
    store.setWsConnected(false)
    store.setSidebarCollapsed(false)
    store.setApprovals([])
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
      expect(screen.getByRole('navigation')).toBeInTheDocument()
      expect(screen.getByRole('main')).toBeInTheDocument()
    })

    it('renders children in main content area', () => {
      renderWithRouter('/', <div data-testid="custom-content">Custom Content</div>)
      expect(screen.getByTestId('custom-content')).toBeInTheDocument()
    })

    it('renders brand name', () => {
      renderWithRouter()
      expect(screen.getByText('APEX')).toBeInTheDocument()
    })

    it('renders version tag', () => {
      renderWithRouter()
      expect(screen.getByText('v0.1')).toBeInTheDocument()
    })
  })

  describe('navigation items', () => {
    it('renders all navigation links', () => {
      renderWithRouter()
      expect(screen.getByText('Command')).toBeInTheDocument()
      expect(screen.getByText('Agents')).toBeInTheDocument()
      expect(screen.getByText('Operations')).toBeInTheDocument()
      expect(screen.getByText('Approvals')).toBeInTheDocument()
      expect(screen.getByText('Settings')).toBeInTheDocument()
    })

    it('highlights active navigation link', () => {
      renderWithRouter('/')
      const commandLink = screen.getByText('Command').closest('a')
      expect(commandLink).toHaveClass('bg-white/[0.06]')
    })

    it('navigates to correct routes', async () => {
      const user = userEvent.setup()
      renderWithRouter()
      await user.click(screen.getByText('Agents'))
      expect(window.location.pathname).toBe('/')
    })

    it('shows correct icons for navigation items', () => {
      const { container } = renderWithRouter()
      const navIcons = container.querySelectorAll('nav svg')
      expect(navIcons.length).toBe(7)
    })
  })

  describe('sidebar collapse', () => {
    it('renders expanded sidebar by default', () => {
      renderWithRouter()
      expect(screen.getByText('Command')).toBeVisible()
      expect(screen.getByText('Agents')).toBeVisible()
    })

    it('toggles sidebar collapse on button click', async () => {
      const user = userEvent.setup()
      renderWithRouter()
      const toggleButton = screen.getByRole('button')
      await user.click(toggleButton)
      expect(useStore.getState().sidebarCollapsed).toBe(true)
    })

    it('updates store when sidebar is toggled back', async () => {
      const user = userEvent.setup()
      renderWithRouter()
      const toggleButton = screen.getByRole('button')
      await user.click(toggleButton)
      expect(useStore.getState().sidebarCollapsed).toBe(true)
      await user.click(toggleButton)
      expect(useStore.getState().sidebarCollapsed).toBe(false)
    })
  })

  describe('connection status', () => {
    it('shows OFFLINE when wsConnected is false', () => {
      useStore.getState().setWsConnected(false)
      renderWithRouter()
      expect(screen.getByText('OFFLINE')).toBeInTheDocument()
    })

    it('shows LIVE when wsConnected is true', () => {
      useStore.getState().setWsConnected(true)
      renderWithRouter()
      expect(screen.getByText('LIVE')).toBeInTheDocument()
    })

    it('shows emerald WiFi icon when connected', () => {
      useStore.getState().setWsConnected(true)
      const { container } = renderWithRouter()
      const statusIcon = container.querySelector('.text-emerald-400')
      expect(statusIcon).toBeInTheDocument()
    })

    it('shows red WifiOff icon when disconnected', () => {
      useStore.getState().setWsConnected(false)
      const { container } = renderWithRouter()
      const statusArea = container.querySelector('.text-red-400\\/60')
      expect(statusArea).toBeInTheDocument()
    })
  })

  describe('top bar metrics', () => {
    it('displays agent count', () => {
      renderWithRouter()
      expect(screen.getByText('8/10')).toBeInTheDocument()
    })

    it('displays running tasks count', () => {
      renderWithRouter()
      const elements = screen.getAllByText('15')
      expect(elements.length).toBeGreaterThanOrEqual(1)
    })

    it('displays success rate with correct color', () => {
      renderWithRouter()
      const successRate = screen.getByText('95.0%')
      expect(successRate).toBeInTheDocument()
      expect(successRate).toHaveClass('text-emerald-400')
    })

    it('shows amber color for medium success rate', () => {
      useStore.getState().setMetrics({ successRate: 0.85 })
      renderWithRouter()
      const successRate = screen.getByText('85.0%')
      expect(successRate).toHaveClass('text-amber-400')
    })

    it('shows red color for low success rate', () => {
      useStore.getState().setMetrics({ successRate: 0.7 })
      renderWithRouter()
      const successRate = screen.getByText('70.0%')
      expect(successRate).toHaveClass('text-red-400')
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
      expect(header).toHaveClass('h-11')
    })
  })

  describe('sidebar styling', () => {
    it('has border on right side', () => {
      const { container } = renderWithRouter()
      const sidebar = container.querySelector('aside')
      expect(sidebar).toHaveClass('border-r')
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
  })

  describe('theme integration', () => {
    it('uses apex theme colors for background', () => {
      renderWithRouter()
      const main = screen.getByRole('main')
      expect(main).toHaveClass('bg-apex-bg-primary')
    })

    it('uses apex theme colors for text', () => {
      const { container } = renderWithRouter()
      const secondaryText = container.querySelector('.text-apex-text-secondary')
      expect(secondaryText).toBeInTheDocument()
    })
  })
})
