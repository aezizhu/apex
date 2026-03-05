import type { Meta, StoryObj } from '@storybook/react'
import {
  Card,
  CardHeader,
  CardContent,
  CardFooter,
  StatCard,
} from './Card'
import { Button } from './Button'
import { Activity, Cpu, DollarSign, Users, MoreVertical, AlertCircle, TrendingUp } from 'lucide-react'

const meta: Meta<typeof Card> = {
  title: 'UI/Card',
  component: Card,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component:
          'A flexible card component with multiple variants, padding options, and interactive states. Includes specialized subcomponents for headers, content, and footers.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'elevated', 'ghost', 'glass', 'glow'],
      description: 'The visual style variant of the card',
    },
    padding: {
      control: 'select',
      options: ['none', 'sm', 'md', 'lg', 'xl'],
      description: 'The padding size of the card',
    },
    interactive: {
      control: 'boolean',
      description: 'Makes the card interactive with hover effects',
    },
  },
}

export default meta
type Story = StoryObj<typeof meta>

// ============================================================================
// Basic Variants
// ============================================================================

export const Default: Story = {
  args: {
    variant: 'default',
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Default Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          This is a basic card with default styling.
        </p>
      </div>
    ),
  },
}

export const Elevated: Story = {
  args: {
    variant: 'elevated',
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Elevated Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          This card has elevated styling with enhanced shadow.
        </p>
      </div>
    ),
  },
}

export const Ghost: Story = {
  args: {
    variant: 'ghost',
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Ghost Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          A transparent card with no background.
        </p>
      </div>
    ),
  },
}

export const Glass: Story = {
  args: {
    variant: 'glass',
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Glass Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          A card with glassmorphism effect and backdrop blur.
        </p>
      </div>
    ),
  },
}

export const Glow: Story = {
  args: {
    variant: 'glow',
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Glow Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          A card with glowing accent border effect.
        </p>
      </div>
    ),
  },
}

export const AllVariants: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4 w-[600px]">
      <Card variant="default">
        <p className="text-sm font-medium">Default</p>
      </Card>
      <Card variant="elevated">
        <p className="text-sm font-medium">Elevated</p>
      </Card>
      <Card variant="ghost">
        <p className="text-sm font-medium">Ghost</p>
      </Card>
      <Card variant="glass">
        <p className="text-sm font-medium">Glass</p>
      </Card>
      <Card variant="glow" className="col-span-2">
        <p className="text-sm font-medium">Glow</p>
      </Card>
    </div>
  ),
}

// ============================================================================
// Padding Variants
// ============================================================================

export const PaddingNone: Story = {
  args: {
    padding: 'none',
    children: <div className="bg-apex-bg-tertiary p-4">No padding on card</div>,
  },
}

export const PaddingSmall: Story = {
  args: {
    padding: 'sm',
    children: <p className="text-sm">Small padding</p>,
  },
}

export const PaddingMedium: Story = {
  args: {
    padding: 'md',
    children: <p className="text-sm">Medium padding (default)</p>,
  },
}

export const PaddingLarge: Story = {
  args: {
    padding: 'lg',
    children: <p className="text-sm">Large padding</p>,
  },
}

export const PaddingExtraLarge: Story = {
  args: {
    padding: 'xl',
    children: <p className="text-sm">Extra large padding</p>,
  },
}

export const AllPaddings: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4 items-start">
      <Card padding="none">
        <div className="p-2 bg-apex-bg-tertiary text-xs">None</div>
      </Card>
      <Card padding="sm">
        <span className="text-xs">Small</span>
      </Card>
      <Card padding="md">
        <span className="text-xs">Medium</span>
      </Card>
      <Card padding="lg">
        <span className="text-xs">Large</span>
      </Card>
      <Card padding="xl">
        <span className="text-xs">XL</span>
      </Card>
    </div>
  ),
}

// ============================================================================
// Interactive Cards
// ============================================================================

export const Interactive: Story = {
  args: {
    interactive: true,
    children: (
      <div>
        <h3 className="text-lg font-semibold text-apex-text-primary">Interactive Card</h3>
        <p className="text-sm text-apex-text-secondary mt-2">
          Hover over this card to see the interactive effect.
        </p>
      </div>
    ),
  },
}

export const InteractiveGrid: Story = {
  render: () => (
    <div className="grid grid-cols-3 gap-4 w-[500px]">
      {['Agent Alpha', 'Agent Beta', 'Agent Gamma', 'Agent Delta', 'Agent Epsilon', 'Agent Zeta'].map(
        (name) => (
          <Card key={name} interactive className="cursor-pointer">
            <div className="text-center">
              <div className="w-8 h-8 rounded-full bg-apex-accent-primary/20 mx-auto mb-2 flex items-center justify-center">
                <Cpu className="h-4 w-4 text-apex-accent-primary" />
              </div>
              <p className="text-sm font-medium text-apex-text-primary">{name}</p>
              <p className="text-xs text-apex-text-tertiary">Online</p>
            </div>
          </Card>
        )
      )}
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A grid of interactive cards, perfect for selectable items.',
      },
    },
  },
}

// ============================================================================
// With Subcomponents
// ============================================================================

export const WithHeader: Story = {
  render: () => (
    <Card className="w-80">
      <CardHeader title="Card Title" description="This is a description of the card content." />
      <CardContent>
        <p className="text-sm text-apex-text-secondary">
          Card content goes here. You can add any content including text, images, or other
          components.
        </p>
      </CardContent>
    </Card>
  ),
}

export const WithHeaderAction: Story = {
  render: () => (
    <Card className="w-80">
      <CardHeader
        title="Metrics Overview"
        description="Last updated 5 minutes ago"
        action={
          <Button variant="ghost" size="icon-sm">
            <MoreVertical className="h-4 w-4" />
          </Button>
        }
      />
      <CardContent>
        <div className="space-y-2">
          <div className="flex justify-between">
            <span className="text-sm text-apex-text-secondary">Total Requests</span>
            <span className="text-sm font-medium">12,543</span>
          </div>
          <div className="flex justify-between">
            <span className="text-sm text-apex-text-secondary">Success Rate</span>
            <span className="text-sm font-medium text-green-500">99.2%</span>
          </div>
        </div>
      </CardContent>
    </Card>
  ),
}

export const WithFooter: Story = {
  render: () => (
    <Card className="w-80">
      <CardHeader title="Confirm Action" description="Are you sure you want to proceed?" />
      <CardContent>
        <p className="text-sm text-apex-text-secondary">
          This action will permanently delete the selected items. This cannot be undone.
        </p>
      </CardContent>
      <CardFooter>
        <Button variant="ghost" size="sm">
          Cancel
        </Button>
        <Button variant="danger" size="sm" className="ml-auto">
          Delete
        </Button>
      </CardFooter>
    </Card>
  ),
}

export const CompleteCard: Story = {
  render: () => (
    <Card variant="elevated" className="w-96">
      <CardHeader
        title="Agent Configuration"
        description="Configure your AI agent settings"
        action={
          <Button variant="ghost" size="icon-sm">
            <MoreVertical className="h-4 w-4" />
          </Button>
        }
      />
      <CardContent>
        <div className="space-y-4">
          <div className="flex items-center gap-3 p-3 rounded-lg bg-apex-bg-tertiary">
            <Cpu className="h-5 w-5 text-apex-accent-primary" />
            <div>
              <p className="text-sm font-medium">Model: GPT-4</p>
              <p className="text-xs text-apex-text-tertiary">Latest version</p>
            </div>
          </div>
          <div className="flex items-center gap-3 p-3 rounded-lg bg-apex-bg-tertiary">
            <Activity className="h-5 w-5 text-green-500" />
            <div>
              <p className="text-sm font-medium">Status: Active</p>
              <p className="text-xs text-apex-text-tertiary">Processing 3 tasks</p>
            </div>
          </div>
        </div>
      </CardContent>
      <CardFooter>
        <Button variant="secondary" size="sm">
          View Logs
        </Button>
        <Button size="sm" className="ml-auto">
          Configure
        </Button>
      </CardFooter>
    </Card>
  ),
}

// ============================================================================
// Stat Cards
// ============================================================================

export const BasicStatCard: Story = {
  render: () => (
    <StatCard
      label="Total Agents"
      value="128"
      change={{ value: 12.5, trend: 'up' }}
      icon={<Users className="h-5 w-5" />}
      className="w-64"
    />
  ),
}

export const StatCardTrendDown: Story = {
  render: () => (
    <StatCard
      label="Error Rate"
      value="2.3%"
      change={{ value: 0.8, trend: 'down' }}
      icon={<AlertCircle className="h-5 w-5" />}
      className="w-64"
    />
  ),
}

export const StatCardNeutral: Story = {
  render: () => (
    <StatCard
      label="Average Latency"
      value="142ms"
      change={{ value: 0, trend: 'neutral' }}
      icon={<Activity className="h-5 w-5" />}
      className="w-64"
    />
  ),
}

export const StatCardGrid: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4 w-[550px]">
      <StatCard
        label="Active Agents"
        value="87"
        change={{ value: 5.2, trend: 'up' }}
        icon={<Cpu className="h-5 w-5" />}
      />
      <StatCard
        label="Total Cost"
        value="$1,234.56"
        change={{ value: 23.1, trend: 'up' }}
        icon={<DollarSign className="h-5 w-5" />}
      />
      <StatCard
        label="Success Rate"
        value="99.2%"
        change={{ value: 0.3, trend: 'up' }}
        icon={<TrendingUp className="h-5 w-5" />}
      />
      <StatCard
        label="Tasks Completed"
        value="12,543"
        change={{ value: 8.7, trend: 'up' }}
        icon={<Activity className="h-5 w-5" />}
      />
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A dashboard-style grid of stat cards showing key metrics.',
      },
    },
  },
}

// ============================================================================
// Complex Layouts
// ============================================================================

export const DashboardPanel: Story = {
  render: () => (
    <Card variant="elevated" padding="none" className="w-[500px]">
      <div className="p-4 border-b border-apex-border-subtle">
        <CardHeader
          title="System Health"
          description="Real-time monitoring dashboard"
          action={<Button variant="ghost" size="sm">Refresh</Button>}
        />
      </div>
      <div className="p-4 space-y-3">
        {[
          { name: 'API Gateway', status: 'healthy', latency: '23ms' },
          { name: 'Database', status: 'healthy', latency: '5ms' },
          { name: 'Cache Layer', status: 'warning', latency: '89ms' },
          { name: 'ML Pipeline', status: 'healthy', latency: '156ms' },
        ].map((service) => (
          <div
            key={service.name}
            className="flex items-center justify-between p-3 rounded-lg bg-apex-bg-tertiary"
          >
            <div className="flex items-center gap-3">
              <div
                className={`w-2 h-2 rounded-full ${
                  service.status === 'healthy' ? 'bg-green-500' : 'bg-yellow-500'
                }`}
              />
              <span className="text-sm font-medium">{service.name}</span>
            </div>
            <span className="text-xs text-apex-text-tertiary font-mono">{service.latency}</span>
          </div>
        ))}
      </div>
      <div className="p-4 border-t border-apex-border-subtle bg-apex-bg-tertiary/50">
        <p className="text-xs text-apex-text-tertiary">Last updated: Just now</p>
      </div>
    </Card>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A complex dashboard panel layout using card with custom padding and sections.',
      },
    },
  },
}

export const NotificationCard: Story = {
  render: () => (
    <Card variant="glow" className="w-80">
      <div className="flex gap-3">
        <div className="p-2 rounded-lg bg-apex-accent-primary/20">
          <AlertCircle className="h-5 w-5 text-apex-accent-primary" />
        </div>
        <div className="flex-1">
          <p className="text-sm font-medium">Approval Required</p>
          <p className="text-xs text-apex-text-secondary mt-1">
            Agent Alpha is requesting permission to execute a high-risk action.
          </p>
          <div className="flex gap-2 mt-3">
            <Button size="xs" variant="secondary">
              Deny
            </Button>
            <Button size="xs">Approve</Button>
          </div>
        </div>
      </div>
    </Card>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A notification card with action buttons and glow effect.',
      },
    },
  },
}
