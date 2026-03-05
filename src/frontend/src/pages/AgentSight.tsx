import { useState, useCallback, useMemo, useEffect } from 'react'
import { motion } from 'framer-motion'
import {
  Search,
  ZoomIn,
  ZoomOut,
  Eye,
} from 'lucide-react'
import { AgentSight as AgentSightGrid } from '../components/AgentSight'
import type { AgentScreen, AgentStatus } from '../components/AgentSight'
import { useStore, selectAgentList } from '../lib/store'
import { agentApi } from '../lib/api'
import { cn } from '../lib/utils'

// ═══════════════════════════════════════════════════════════════════════════════
// Agent screen data builder from real store data
// ═══════════════════════════════════════════════════════════════════════════════

const STATUS_ACTIONS: Record<string, string> = {
  busy: 'Processing task...',
  idle: 'Waiting for assignment',
  error: 'Error — awaiting retry',
  paused: 'Paused by operator',
}

function buildAgentScreens(agents: ReturnType<typeof selectAgentList>): AgentScreen[] {
  return agents.map((agent) => ({
    agentId: agent.id,
    agentName: agent.name,
    status: agent.status,
    currentAction: STATUS_ACTIONS[agent.status] ?? 'Unknown',
    lastToolCall: undefined,
    screenContent: agent.status === 'busy'
      ? `Agent ${agent.name} is actively processing. Model: ${agent.model}. Load: ${agent.currentLoad}/${agent.maxLoad}.`
      : agent.status === 'error'
        ? `Agent encountered an error. Success rate: ${(agent.successRate * 100).toFixed(1)}%.`
        : `Agent is ${agent.status}. Model: ${agent.model}. Reputation: ${(agent.reputationScore * 100).toFixed(0)}%.`,
    focusArea: agent.status === 'busy' ? `Load ${agent.currentLoad}/${agent.maxLoad}` : undefined,
    tokensUsed: agent.totalTokens,
    costSoFar: agent.totalCost,
    startedAt: agent.createdAt ?? new Date().toISOString(),
  }))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Page Component
// ═══════════════════════════════════════════════════════════════════════════════

type ZoomLevel = 1 | 2 | 4

export default function AgentSightPage() {
  const agents = useStore(selectAgentList)
  const setAgents = useStore((s) => s.setAgents)
  const wsConnected = useStore((s) => s.wsConnected)

  // Fetch agents on mount
  useEffect(() => {
    async function fetchAgents() {
      try {
        const response = await agentApi.listRaw()
        if (Array.isArray(response.data)) {
          setAgents(response.data)
        }
      } catch {
        // WebSocket will keep data flowing
      }
    }
    fetchAgents()
  }, [setAgents])

  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<AgentStatus | 'all'>('all')
  const [zoomLevel, setZoomLevel] = useState<ZoomLevel>(1)

  // Build screen data from store agents
  const agentScreens = useMemo(() => buildAgentScreens(agents), [agents])

  // Filter
  const filteredScreens = useMemo(() => {
    return agentScreens.filter((screen) => {
      const matchesSearch =
        searchQuery === '' ||
        screen.agentName.toLowerCase().includes(searchQuery.toLowerCase()) ||
        screen.agentId.toLowerCase().includes(searchQuery.toLowerCase())
      const matchesStatus = statusFilter === 'all' || screen.status === statusFilter
      return matchesSearch && matchesStatus
    })
  }, [agentScreens, searchQuery, statusFilter])

  const statusCounts = useMemo(() => {
    return {
      all: agentScreens.length,
      idle: agentScreens.filter((a) => a.status === 'idle').length,
      busy: agentScreens.filter((a) => a.status === 'busy').length,
      error: agentScreens.filter((a) => a.status === 'error').length,
      paused: agentScreens.filter((a) => a.status === 'paused').length,
    }
  }, [agentScreens])

  const cycleZoom = useCallback(() => {
    setZoomLevel((prev) => {
      if (prev === 1) return 2
      if (prev === 2) return 4
      return 1
    })
  }, [])

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-3">
            <Eye size={28} className="text-apex-accent-primary" />
            <h1 className="text-2xl font-bold">Agent Sight</h1>
          </div>
          <p className="text-apex-text-secondary mt-1">
            Live screen feeds from your agent swarm
          </p>
        </div>
        <div className="flex items-center gap-2 text-sm text-apex-text-tertiary">
          <div
            className={cn(
              'w-2 h-2 rounded-full',
              wsConnected ? 'bg-green-500' : 'bg-red-500'
            )}
          />
          <span>{wsConnected ? 'Live' : 'Offline'}</span>
          <span className="text-apex-border-default mx-1">|</span>
          <span>{filteredScreens.length} agents</span>
        </div>
      </div>

      {/* Controls */}
      <div className="flex flex-wrap items-center gap-4">
        {/* Search */}
        <div className="relative flex-1 min-w-[200px] max-w-md">
          <Search
            size={18}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-apex-text-tertiary"
          />
          <input
            type="text"
            placeholder="Search agents..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
          />
        </div>

        {/* Status Filter */}
        <div className="flex items-center gap-2">
          {(['all', 'idle', 'busy', 'error', 'paused'] as const).map((status) => (
            <button
              key={status}
              onClick={() => setStatusFilter(status)}
              className={cn(
                'px-3 py-1.5 rounded-lg text-sm font-medium transition-colors',
                statusFilter === status
                  ? 'bg-apex-accent-primary text-white'
                  : 'bg-apex-bg-secondary text-apex-text-secondary hover:bg-apex-bg-tertiary'
              )}
            >
              {status.charAt(0).toUpperCase() + status.slice(1)}
              <span className="ml-1 text-xs opacity-70">({statusCounts[status]})</span>
            </button>
          ))}
        </div>

        {/* Zoom Control */}
        <div className="flex items-center gap-1 bg-apex-bg-secondary rounded-lg p-1">
          <button
            onClick={() => setZoomLevel(Math.max(1, zoomLevel === 4 ? 2 : 1) as ZoomLevel)}
            className={cn(
              'p-2 rounded-md transition-colors',
              'text-apex-text-tertiary hover:text-apex-text-secondary hover:bg-apex-bg-tertiary'
            )}
            title="Zoom out"
          >
            <ZoomOut size={18} />
          </button>
          <button
            onClick={cycleZoom}
            className="px-2 py-1 text-xs font-medium text-apex-text-secondary min-w-[32px] text-center"
            title="Toggle zoom"
          >
            {zoomLevel}x
          </button>
          <button
            onClick={() => setZoomLevel(Math.min(4, zoomLevel === 1 ? 2 : 4) as ZoomLevel)}
            className={cn(
              'p-2 rounded-md transition-colors',
              'text-apex-text-tertiary hover:text-apex-text-secondary hover:bg-apex-bg-tertiary'
            )}
            title="Zoom in"
          >
            <ZoomIn size={18} />
          </button>
        </div>
      </div>

      {/* Grid */}
      {filteredScreens.length > 0 ? (
        <AgentSightGrid agents={filteredScreens} zoomLevel={zoomLevel} />
      ) : (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="flex flex-col items-center justify-center py-20 text-apex-text-tertiary"
        >
          <Eye size={48} className="mb-4 opacity-30" />
          <p className="text-lg font-medium">No agents found</p>
          <p className="text-sm mt-1">
            {agents.length === 0
              ? 'Waiting for agents to connect...'
              : 'Try adjusting your search or filter.'}
          </p>
        </motion.div>
      )}
    </div>
  )
}
