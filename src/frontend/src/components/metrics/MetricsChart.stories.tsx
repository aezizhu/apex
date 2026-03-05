import type { Meta, StoryObj } from '@storybook/react'
import MetricsChart from './MetricsChart'

const meta: Meta<typeof MetricsChart> = {
  title: 'Metrics/MetricsChart',
  component: MetricsChart,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component:
          'A dual-axis time series chart displaying task completion rates and average latency over time. Built with Plotly.js for interactive data visualization with hover tooltips and responsive design.',
      },
    },
  },
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '400px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
}

export default meta
type Story = StoryObj<typeof meta>

// ============================================================================
// Basic Examples
// ============================================================================

export const Default: Story = {
  parameters: {
    docs: {
      description: {
        story: 'Default metrics chart with simulated task completion and latency data over the last 50 minutes.',
      },
    },
  },
}

export const InCard: Story = {
  render: () => (
    <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle overflow-hidden">
      <div className="p-4 border-b border-apex-border-subtle">
        <h3 className="text-lg font-semibold text-apex-text-primary">System Metrics</h3>
        <p className="text-sm text-apex-text-secondary">Real-time performance monitoring</p>
      </div>
      <div style={{ height: '300px' }}>
        <MetricsChart />
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '800px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Metrics chart embedded within a card component with header.',
      },
    },
  },
}

export const FullWidth: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '500px', background: '#0a0a0f' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Full-width chart suitable for dashboard main panels.',
      },
    },
  },
}

export const CompactHeight: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '250px', background: '#0a0a0f', padding: '0.5rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Compact height chart suitable for smaller dashboard widgets.',
      },
    },
  },
}

// ============================================================================
// Dashboard Layouts
// ============================================================================

export const DashboardWidget: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4">
      <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h4 className="text-sm font-semibold text-apex-text-primary">Task Throughput</h4>
            <p className="text-xs text-apex-text-tertiary">Tasks per minute</p>
          </div>
          <div className="text-right">
            <p className="text-lg font-bold text-green-500">12.5</p>
            <p className="text-xs text-apex-text-tertiary">+8% vs avg</p>
          </div>
        </div>
        <div style={{ height: '200px' }}>
          <MetricsChart />
        </div>
      </div>
      <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h4 className="text-sm font-semibold text-apex-text-primary">Response Latency</h4>
            <p className="text-xs text-apex-text-tertiary">Average ms</p>
          </div>
          <div className="text-right">
            <p className="text-lg font-bold text-blue-500">1,234</p>
            <p className="text-xs text-apex-text-tertiary">-12% vs avg</p>
          </div>
        </div>
        <div style={{ height: '200px' }}>
          <MetricsChart />
        </div>
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '900px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Two side-by-side dashboard widgets with metrics charts.',
      },
    },
  },
}

export const MultipleCharts: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
        <h4 className="text-sm font-semibold text-apex-text-primary mb-2">Primary Metrics</h4>
        <div style={{ height: '250px' }}>
          <MetricsChart />
        </div>
      </div>
      <div className="grid grid-cols-3 gap-4">
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-3">
          <h5 className="text-xs font-medium text-apex-text-secondary mb-2">Agent Pool A</h5>
          <div style={{ height: '150px' }}>
            <MetricsChart />
          </div>
        </div>
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-3">
          <h5 className="text-xs font-medium text-apex-text-secondary mb-2">Agent Pool B</h5>
          <div style={{ height: '150px' }}>
            <MetricsChart />
          </div>
        </div>
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-3">
          <h5 className="text-xs font-medium text-apex-text-secondary mb-2">Agent Pool C</h5>
          <div style={{ height: '150px' }}>
            <MetricsChart />
          </div>
        </div>
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '1000px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Multiple charts showing aggregate and per-pool metrics.',
      },
    },
  },
}

// ============================================================================
// Contextual Examples
// ============================================================================

export const WithControls: Story = {
  render: () => (
    <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle overflow-hidden">
      <div className="p-4 border-b border-apex-border-subtle flex items-center justify-between">
        <div>
          <h3 className="text-lg font-semibold text-apex-text-primary">Performance Metrics</h3>
          <p className="text-sm text-apex-text-secondary">Last 50 minutes</p>
        </div>
        <div className="flex gap-2">
          <button className="px-3 py-1.5 text-xs bg-apex-bg-tertiary rounded-lg hover:bg-apex-bg-elevated transition-colors text-apex-text-secondary">
            1h
          </button>
          <button className="px-3 py-1.5 text-xs bg-apex-accent-primary text-white rounded-lg">
            6h
          </button>
          <button className="px-3 py-1.5 text-xs bg-apex-bg-tertiary rounded-lg hover:bg-apex-bg-elevated transition-colors text-apex-text-secondary">
            24h
          </button>
          <button className="px-3 py-1.5 text-xs bg-apex-bg-tertiary rounded-lg hover:bg-apex-bg-elevated transition-colors text-apex-text-secondary">
            7d
          </button>
        </div>
      </div>
      <div style={{ height: '350px' }}>
        <MetricsChart />
      </div>
      <div className="p-4 border-t border-apex-border-subtle flex items-center justify-between bg-apex-bg-tertiary/30">
        <div className="flex gap-6">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-green-500" />
            <span className="text-xs text-apex-text-secondary">Tasks Completed</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-blue-500" />
            <span className="text-xs text-apex-text-secondary">Avg Latency (ms)</span>
          </div>
        </div>
        <button className="text-xs text-apex-accent-primary hover:underline">
          Export Data
        </button>
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '900px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Chart with time range controls and legend.',
      },
    },
  },
}

export const StatusPanel: Story = {
  render: () => (
    <div className="grid grid-cols-4 gap-4">
      <div className="col-span-3 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
        <h3 className="text-lg font-semibold text-apex-text-primary mb-4">System Overview</h3>
        <div style={{ height: '300px' }}>
          <MetricsChart />
        </div>
      </div>
      <div className="space-y-4">
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <p className="text-xs text-apex-text-tertiary mb-1">Uptime</p>
          <p className="text-2xl font-bold text-green-500">99.9%</p>
        </div>
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <p className="text-xs text-apex-text-tertiary mb-1">Active Agents</p>
          <p className="text-2xl font-bold text-apex-text-primary">87</p>
        </div>
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <p className="text-xs text-apex-text-tertiary mb-1">Tasks/min</p>
          <p className="text-2xl font-bold text-blue-500">12.5</p>
        </div>
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <p className="text-xs text-apex-text-tertiary mb-1">Error Rate</p>
          <p className="text-2xl font-bold text-red-500">0.3%</p>
        </div>
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '1100px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Chart alongside status indicator panels.',
      },
    },
  },
}

// ============================================================================
// Responsive Examples
// ============================================================================

export const Mobile: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: '375px', height: '300px', background: '#0a0a0f', padding: '0.5rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    viewport: {
      defaultViewport: 'mobile',
    },
    docs: {
      description: {
        story: 'Chart optimized for mobile viewport.',
      },
    },
  },
}

export const Tablet: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: '768px', height: '350px', background: '#0a0a0f', padding: '1rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
    docs: {
      description: {
        story: 'Chart optimized for tablet viewport.',
      },
    },
  },
}

// ============================================================================
// Theme Variations
// ============================================================================

export const WithGlassEffect: Story = {
  render: () => (
    <div className="relative">
      <div className="absolute inset-0 bg-gradient-to-br from-blue-500/10 to-purple-500/10 rounded-xl" />
      <div className="relative glass rounded-xl p-4">
        <h3 className="text-lg font-semibold text-apex-text-primary mb-4">Metrics</h3>
        <div style={{ height: '300px' }}>
          <MetricsChart />
        </div>
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '800px', background: '#0a0a0f', padding: '2rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Chart with glassmorphism background effect.',
      },
    },
  },
}

export const GlowEffect: Story = {
  render: () => (
    <div className="bg-apex-bg-secondary rounded-xl border border-apex-accent-primary/30 shadow-glow-sm p-4">
      <h3 className="text-lg font-semibold text-apex-text-primary mb-4">Live Metrics</h3>
      <div style={{ height: '300px' }}>
        <MetricsChart />
      </div>
    </div>
  ),
  decorators: [
    (Story) => (
      <div style={{ width: '100%', maxWidth: '800px', background: '#0a0a0f', padding: '2rem' }}>
        <Story />
      </div>
    ),
  ],
  parameters: {
    docs: {
      description: {
        story: 'Chart with glowing border effect.',
      },
    },
  },
}
