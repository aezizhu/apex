import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Settings from '@/pages/Settings'
import { useStore } from '@/lib/store'
import { settingsApi } from '@/lib/api'

// Mock framer-motion
vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, className, ...props }: any) => (
      <div className={className} {...props}>
        {children}
      </div>
    ),
  },
}))

// Mock react-hot-toast
vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}))

const mockApiKeys = [
  { id: 'k1', provider: 'openai', maskedKey: 'sk-...abc', createdAt: '2024-01-01' },
]

// Mock API calls — default returns empty
vi.mock('@/lib/api', () => ({
  settingsApi: {
    get: vi.fn().mockResolvedValue({ data: null }),
    update: vi.fn().mockResolvedValue({}),
    getApiKeys: vi.fn().mockResolvedValue({ data: [] }),
    createApiKey: vi.fn().mockResolvedValue({ data: null }),
    revokeApiKey: vi.fn().mockResolvedValue({}),
  },
}))

describe('Settings', () => {
  beforeEach(() => {
    const store = useStore.getState()
    store.setSettings({
      maxConcurrentAgents: 10,
      defaultModel: 'gpt-4o-mini',
      autoRetryEnabled: false,
      maxRetries: 3,
      logLevel: 'info',
    })
    store.setApiKeys([])
  })

  describe('section navigation', () => {
    it('renders all section buttons', () => {
      render(<Settings />)
      expect(screen.getByText('General')).toBeInTheDocument()
      expect(screen.getByText('API Keys')).toBeInTheDocument()
      expect(screen.getByText('Resource Limits')).toBeInTheDocument()
      expect(screen.getByText('Notifications')).toBeInTheDocument()
      expect(screen.getByText('Appearance')).toBeInTheDocument()
    })

    it('shows General section by default', () => {
      render(<Settings />)
      expect(screen.getByText('General Settings')).toBeInTheDocument()
    })

    it('switches to API Keys section', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('API Keys'))
      expect(screen.getByText('Manage LLM provider credentials')).toBeInTheDocument()
    })

    it('switches to Resource Limits section', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Resource Limits'))
      expect(screen.getByText('Set default resource constraints for tasks')).toBeInTheDocument()
    })

    it('switches to Notifications section', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Notifications'))
      expect(screen.getByText('Configure alerting preferences')).toBeInTheDocument()
    })

    it('switches to Appearance section', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Appearance'))
      expect(screen.getByText('Customize the dashboard look and feel')).toBeInTheDocument()
    })
  })

  describe('general section', () => {
    it('renders Max Concurrent Agents input', () => {
      render(<Settings />)
      expect(screen.getByText('Max Concurrent Agents')).toBeInTheDocument()
    })

    it('renders Default Model select', () => {
      render(<Settings />)
      expect(screen.getByText('Default Model')).toBeInTheDocument()
    })

    it('renders model options', () => {
      render(<Settings />)
      expect(screen.getByText('GPT-4o Mini')).toBeInTheDocument()
      expect(screen.getByText('GPT-4o')).toBeInTheDocument()
      expect(screen.getByText('Claude 3.5 Haiku')).toBeInTheDocument()
      expect(screen.getByText('Claude 3.5 Sonnet')).toBeInTheDocument()
    })

    it('renders Auto-Retry toggle', () => {
      render(<Settings />)
      expect(screen.getByText('Auto-Retry Failed Tasks')).toBeInTheDocument()
    })

    it('renders Log Level select', () => {
      render(<Settings />)
      expect(screen.getByText('Log Level')).toBeInTheDocument()
    })

    it('renders Save Changes button', () => {
      render(<Settings />)
      expect(screen.getByText('Save Changes')).toBeInTheDocument()
    })

    it('does not show Max Retries when auto-retry is disabled', () => {
      render(<Settings />)
      expect(screen.queryByText('Max Retries')).not.toBeInTheDocument()
    })

    it('shows Max Retries when auto-retry is enabled', () => {
      useStore.getState().setSettings({ autoRetryEnabled: true })
      render(<Settings />)
      expect(screen.getByText('Max Retries')).toBeInTheDocument()
    })
  })

  describe('API keys section', () => {
    it('shows empty state when no keys', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('API Keys'))
      expect(screen.getByText('No API keys configured')).toBeInTheDocument()
    })

    it('shows Add Key button', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('API Keys'))
      expect(screen.getByText('Add Key')).toBeInTheDocument()
    })

    it('does not show Save Changes in API Keys section', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('API Keys'))
      expect(screen.queryByText('Save Changes')).not.toBeInTheDocument()
    })

    it('shows existing keys when API returns them', async () => {
      vi.mocked(settingsApi.getApiKeys).mockResolvedValueOnce({ data: mockApiKeys })
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('API Keys'))
      await waitFor(() => {
        expect(screen.getByText('openai')).toBeInTheDocument()
      })
      expect(screen.getByText('sk-...abc')).toBeInTheDocument()
    })
  })

  describe('appearance section', () => {
    it('shows theme buttons', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Appearance'))
      expect(screen.getByText('dark')).toBeInTheDocument()
      expect(screen.getByText('light')).toBeInTheDocument()
      expect(screen.getByText('system')).toBeInTheDocument()
    })

    it('shows Compact Mode toggle', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Appearance'))
      expect(screen.getByText('Compact Mode')).toBeInTheDocument()
    })
  })

  describe('resource limits section', () => {
    it('shows limit inputs', async () => {
      const user = userEvent.setup()
      render(<Settings />)

      await user.click(screen.getByText('Resource Limits'))
      expect(screen.getByText('Default Token Limit')).toBeInTheDocument()
      expect(screen.getByText('Default Cost Limit ($)')).toBeInTheDocument()
      expect(screen.getByText('Default Time Limit (seconds)')).toBeInTheDocument()
      expect(screen.getByText('Circuit Breaker Threshold')).toBeInTheDocument()
    })
  })
})
