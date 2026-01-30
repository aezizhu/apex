import type { Meta, StoryObj } from '@storybook/react'
import { fn } from '@storybook/test'
import AgentGrid from './AgentGrid'
import { useStore, Agent } from '../../lib/store'
import { useEffect } from 'react'

// Mock data generator
const generateMockAgents = (count: number, statusDistribution?: Partial<Record<Agent['status'], number>>): Agent[] => {
  const models = ['gpt-4-turbo', 'gpt-4', 'gpt-3.5-turbo', 'claude-3-opus', 'claude-3-sonnet']

  const distribution = statusDistribution || {
    busy: 0.4,
    idle: 0.35,
    error: 0.1,
    paused: 0.15,
  }

  return Array.from({ length: count }, (_, i) => {
    // Determine status based on distribution
    const rand = Math.random()
    let status: Agent['status'] = 'idle'
    let cumulative = 0
    for (const [s, prob] of Object.entries(distribution)) {
      cumulative += prob as number
      if (rand < cumulative) {
        status = s as Agent['status']
        break
      }
    }

    const successRate = status === 'error'
      ? Math.random() * 0.5 + 0.3
      : Math.random() * 0.3 + 0.7

    return {
      id: `agent-${i + 1}`,
      name: `Agent ${String.fromCharCode(65 + (i % 26))}${Math.floor(i / 26) || ''}`,
      model: models[Math.floor(Math.random() * models.length)] ?? 'gpt-4',
      status,
      currentLoad: status === 'busy' ? Math.floor(Math.random() * 8) + 1 : 0,
      maxLoad: 10,
      successRate,
      reputationScore: Math.random() * 0.4 + 0.6,
      totalTokens: Math.floor(Math.random() * 1000000),
      totalCost: Math.random() * 50,
      confidence: successRate,
    }
  })
}

// Wrapper component to populate store with mock data
const AgentGridWithMockData = ({
  agents,
  ...props
}: {
  agents: Agent[]
} & React.ComponentProps<typeof AgentGrid>) => {
  const setAgents = useStore((s) => s.setAgents)

  useEffect(() => {
    setAgents(agents)
  }, [agents, setAgents])

  return <AgentGrid {...props} />
}

const meta: Meta<typeof AgentGrid> = {
  title: 'Agents/AgentGrid',
  component: AgentGrid,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component:
          'A hexagonal grid visualization of AI agents. Each hexagon represents an agent, with color coding for status and confidence levels. Supports selection, hover interactions, and displays agent details on hover.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    maxAgents: {
      control: { type: 'number', min: 1, max: 500 },
      description: 'Maximum number of agents to display',
    },
    onAgentSelect: {
      action: 'agent-selected',
      description: 'Callback when an agent is selected',
    },
  },
  args: {
    onAgentSelect: fn(),
  },
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '600px', background: '#0a0a0f' }}>
        <Story />
      </div>
    ),
  ],
}

export default meta
type Story = StoryObj<typeof AgentGrid>

// ============================================================================
// Basic Examples
// ============================================================================

export const Default: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(50)}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Default agent grid with 50 agents and mixed statuses.',
      },
    },
  },
}

export const SmallGrid: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(12)}
      maxAgents={12}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'A smaller grid with 12 agents, suitable for compact views.',
      },
    },
  },
}

export const LargeGrid: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(200)}
      maxAgents={200}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'A large grid with 200 agents, demonstrating scalability.',
      },
    },
  },
}

export const MaxCapacity: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(500)}
      maxAgents={500}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Maximum capacity grid with 500 agents.',
      },
    },
  },
}

// ============================================================================
// Status Distributions
// ============================================================================

export const AllBusy: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(50, { busy: 1, idle: 0, error: 0, paused: 0 })}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'All agents in busy state, showing high activity.',
      },
    },
  },
}

export const AllIdle: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(50, { busy: 0, idle: 1, error: 0, paused: 0 })}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'All agents in idle state, showing no activity.',
      },
    },
  },
}

export const HighErrorRate: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(50, { busy: 0.2, idle: 0.2, error: 0.5, paused: 0.1 })}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'High error rate scenario with 50% of agents in error state.',
      },
    },
  },
}

export const MostlyPaused: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(50, { busy: 0.1, idle: 0.1, error: 0.05, paused: 0.75 })}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Most agents paused, simulating a maintenance window.',
      },
    },
  },
}

export const BalancedLoad: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(100, { busy: 0.25, idle: 0.25, error: 0.25, paused: 0.25 })}
      maxAgents={100}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Evenly distributed status across all agents.',
      },
    },
  },
}

// ============================================================================
// Performance Scenarios
// ============================================================================

export const HighPerformingFleet: Story = {
  render: (args) => {
    const agents = generateMockAgents(75, { busy: 0.6, idle: 0.35, error: 0.02, paused: 0.03 })
    // Override to have high success rates
    agents.forEach(agent => {
      if (agent.status !== 'error') {
        agent.successRate = Math.random() * 0.1 + 0.9 // 90-100%
        agent.confidence = agent.successRate
      }
    })
    return <AgentGridWithMockData agents={agents} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'High-performing fleet with 60% busy agents and 2% error rate.',
      },
    },
  },
}

export const DegradedPerformance: Story = {
  render: (args) => {
    const agents = generateMockAgents(75, { busy: 0.2, idle: 0.3, error: 0.35, paused: 0.15 })
    // Override to have lower success rates
    agents.forEach(agent => {
      agent.successRate = Math.random() * 0.4 + 0.4 // 40-80%
      agent.confidence = agent.successRate
    })
    return <AgentGridWithMockData agents={agents} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Degraded system with high error rates and low confidence scores.',
      },
    },
  },
}

// ============================================================================
// Interaction Examples
// ============================================================================

export const WithSelection: Story = {
  render: (args) => {
    const agents = generateMockAgents(30)
    return (
      <AgentGridWithMockData
        agents={agents}
        maxAgents={30}
        onAgentSelect={(agent) => {
          console.log('Selected agent:', agent)
          args.onAgentSelect?.(agent)
        }}
        {...args}
      />
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Click on any agent hexagon to select it. The selected agent will have a highlighted border.',
      },
    },
  },
}

export const HoverInteraction: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(40)}
      maxAgents={40}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Hover over any agent to see detailed information including model, load, success rate, tokens, and cost.',
      },
    },
  },
}

// ============================================================================
// Edge Cases
// ============================================================================

export const SingleAgent: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(1)}
      maxAgents={1}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Grid with a single agent.',
      },
    },
  },
}

export const TruncatedDisplay: Story = {
  render: (args) => (
    <AgentGridWithMockData
      agents={generateMockAgents(200)}
      maxAgents={50}
      {...args}
    />
  ),
  parameters: {
    docs: {
      description: {
        story: 'Grid with 200 agents but maxAgents set to 50, demonstrating truncation.',
      },
    },
  },
}

export const FullyLoadedAgents: Story = {
  render: (args) => {
    const agents = generateMockAgents(40, { busy: 1, idle: 0, error: 0, paused: 0 })
    // Set all agents to max load
    agents.forEach(agent => {
      agent.currentLoad = agent.maxLoad
    })
    return <AgentGridWithMockData agents={agents} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'All agents at maximum load capacity.',
      },
    },
  },
}

export const MixedModels: Story = {
  render: (args) => {
    const models = ['gpt-4-turbo', 'claude-3-opus', 'gemini-pro', 'llama-2-70b', 'mistral-large']
    const agents = generateMockAgents(50)
    // Assign specific models in groups
    agents.forEach((agent, i) => {
      agent.model = models[i % models.length] ?? 'gpt-4'
    })
    return <AgentGridWithMockData agents={agents} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Fleet with a mix of different AI models.',
      },
    },
  },
}

// ============================================================================
// Real-World Scenarios
// ============================================================================

export const PeakTraffic: Story = {
  render: (args) => {
    const agents = generateMockAgents(150, { busy: 0.85, idle: 0.05, error: 0.05, paused: 0.05 })
    // High load on busy agents
    agents.forEach(agent => {
      if (agent.status === 'busy') {
        agent.currentLoad = Math.floor(Math.random() * 3) + 7 // 7-10
        agent.totalTokens = Math.floor(Math.random() * 5000000) + 500000
        agent.totalCost = Math.random() * 200 + 50
      }
    })
    return <AgentGridWithMockData agents={agents} maxAgents={150} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Peak traffic scenario with 85% of agents busy at high load.',
      },
    },
  },
}

export const NightShift: Story = {
  render: (args) => {
    const agents = generateMockAgents(100, { busy: 0.15, idle: 0.7, error: 0.03, paused: 0.12 })
    // Low activity
    agents.forEach(agent => {
      if (agent.status === 'busy') {
        agent.currentLoad = Math.floor(Math.random() * 3) + 1 // 1-3
      }
    })
    return <AgentGridWithMockData agents={agents} maxAgents={100} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Low-traffic period with most agents idle.',
      },
    },
  },
}

export const ScaleOutEvent: Story = {
  render: (args) => {
    // Mix of new (lower stats) and established agents (higher stats)
    const agents = generateMockAgents(120, { busy: 0.5, idle: 0.4, error: 0.05, paused: 0.05 })
    agents.forEach((agent, i) => {
      if (i >= 80) {
        // New agents
        agent.totalTokens = Math.floor(Math.random() * 10000)
        agent.totalCost = Math.random() * 2
        agent.successRate = Math.random() * 0.2 + 0.8
        agent.reputationScore = Math.random() * 0.2 + 0.5
      } else {
        // Established agents
        agent.totalTokens = Math.floor(Math.random() * 2000000) + 100000
        agent.totalCost = Math.random() * 100 + 20
      }
    })
    return <AgentGridWithMockData agents={agents} maxAgents={120} {...args} />
  },
  parameters: {
    docs: {
      description: {
        story: 'Scale-out event with new agents (40) joining the existing fleet (80).',
      },
    },
  },
}
