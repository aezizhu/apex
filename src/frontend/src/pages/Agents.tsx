import { useState, useEffect, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Plus,
  Search,
  Grid,
  List,
  MoreVertical,
  Play,
  Pause,
} from 'lucide-react'
import { useStore, selectAgentList, type Agent } from '../lib/store'
import { cn, formatCost } from '../lib/utils'
import { agentApi } from '../lib/api'
import AgentGrid from '../components/agents/AgentGrid'
import { InterventionPanel } from '../components/InterventionPanel'
import toast from 'react-hot-toast'

type ViewMode = 'grid' | 'list'

export default function Agents() {
  const agents = useStore(selectAgentList)
  const setAgents = useStore((s) => s.setAgents)
  const [viewMode, setViewMode] = useState<ViewMode>('grid')
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<Agent['status'] | 'all'>('all')
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(null)

  // Fetch agents on mount
  const fetchAgents = useCallback(async () => {
    try {
      const response = await agentApi.listRaw()
      if (Array.isArray(response.data)) {
        setAgents(response.data)
      }
    } catch {
      // WebSocket will keep data flowing
    }
  }, [setAgents])

  useEffect(() => {
    fetchAgents()
  }, [fetchAgents])

  const handlePause = useCallback(async (agentId: string) => {
    try {
      await agentApi.pause(agentId)
      toast.success('Agent paused')
      fetchAgents()
    } catch {
      toast.error('Failed to pause agent')
    }
  }, [fetchAgents])

  const handleResume = useCallback(async (agentId: string) => {
    try {
      await agentApi.resume(agentId)
      toast.success('Agent resumed')
      fetchAgents()
    } catch {
      toast.error('Failed to resume agent')
    }
  }, [fetchAgents])

  const handleKill = useCallback(async (agentId: string) => {
    try {
      await agentApi.delete(agentId)
      toast.success('Agent terminated')
      setSelectedAgent(null)
      fetchAgents()
    } catch {
      toast.error('Failed to terminate agent')
    }
  }, [fetchAgents])

  const filteredAgents = agents.filter((agent) => {
    const matchesSearch = agent.name.toLowerCase().includes(searchQuery.toLowerCase())
    const matchesStatus = statusFilter === 'all' || agent.status === statusFilter
    return matchesSearch && matchesStatus
  })

  const statusCounts = {
    all: agents.length,
    idle: agents.filter((a) => a.status === 'idle').length,
    busy: agents.filter((a) => a.status === 'busy').length,
    error: agents.filter((a) => a.status === 'error').length,
    paused: agents.filter((a) => a.status === 'paused').length,
  }

  const handleAgentClick = (agent: Agent) => {
    setSelectedAgent(selectedAgent?.id === agent.id ? null : agent)
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Agents</h1>
          <p className="text-apex-text-secondary">
            Manage and monitor your agent swarm
          </p>
        </div>
        <button className="flex items-center gap-2 px-4 py-2 bg-apex-accent-primary hover:bg-blue-600 text-white rounded-lg transition-colors">
          <Plus size={18} />
          Register Agent
        </button>
      </div>

      {/* Filters */}
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

        {/* View Toggle */}
        <div className="flex items-center gap-1 bg-apex-bg-secondary rounded-lg p-1">
          <button
            onClick={() => setViewMode('grid')}
            className={cn(
              'p-2 rounded-md transition-colors',
              viewMode === 'grid'
                ? 'bg-apex-bg-tertiary text-apex-text-primary'
                : 'text-apex-text-tertiary hover:text-apex-text-secondary'
            )}
          >
            <Grid size={18} />
          </button>
          <button
            onClick={() => setViewMode('list')}
            className={cn(
              'p-2 rounded-md transition-colors',
              viewMode === 'list'
                ? 'bg-apex-bg-tertiary text-apex-text-primary'
                : 'text-apex-text-tertiary hover:text-apex-text-secondary'
            )}
          >
            <List size={18} />
          </button>
        </div>
      </div>

      {/* Content */}
      {viewMode === 'grid' ? (
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4 h-[600px]">
          <AgentGrid maxAgents={500} />
        </div>
      ) : (
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle overflow-hidden">
          {/* Table Header */}
          <div className="grid grid-cols-[1fr_120px_100px_100px_100px_100px_80px] gap-4 px-6 py-3 bg-apex-bg-tertiary text-sm font-medium text-apex-text-secondary border-b border-apex-border-subtle">
            <div>Agent</div>
            <div>Model</div>
            <div>Status</div>
            <div>Load</div>
            <div>Success</div>
            <div>Cost</div>
            <div></div>
          </div>

          {/* Table Body */}
          <div className="divide-y divide-apex-border-subtle">
            <AnimatePresence>
              {filteredAgents.map((agent, i) => (
                <motion.div
                  key={agent.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ delay: i * 0.02 }}
                  className={cn(
                    'grid grid-cols-[1fr_120px_100px_100px_100px_100px_80px] gap-4 px-6 py-4 hover:bg-apex-bg-tertiary/50 transition-colors cursor-pointer',
                    selectedAgent?.id === agent.id && 'bg-apex-bg-tertiary/70 border-l-2 border-l-apex-accent-primary'
                  )}
                  onClick={() => handleAgentClick(agent)}
                >
                  {/* Agent Name */}
                  <div className="flex items-center gap-3">
                    <div
                      className={cn(
                        'w-10 h-10 rounded-lg flex items-center justify-center text-white font-semibold',
                        agent.status === 'busy' && 'bg-blue-500',
                        agent.status === 'idle' && 'bg-gray-500',
                        agent.status === 'error' && 'bg-red-500',
                        agent.status === 'paused' && 'bg-yellow-500'
                      )}
                    >
                      {agent.name.charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <div className="font-medium">{agent.name}</div>
                      <div className="text-xs text-apex-text-tertiary font-mono">
                        {agent.id.slice(0, 8)}
                      </div>
                    </div>
                  </div>

                  {/* Model */}
                  <div className="flex items-center">
                    <span className="text-sm font-mono text-apex-text-secondary">
                      {agent.model}
                    </span>
                  </div>

                  {/* Status */}
                  <div className="flex items-center">
                    <span
                      className={cn(
                        'px-2 py-1 rounded text-xs font-medium',
                        agent.status === 'busy' && 'bg-blue-500/10 text-blue-500',
                        agent.status === 'idle' && 'bg-gray-500/10 text-gray-400',
                        agent.status === 'error' && 'bg-red-500/10 text-red-500',
                        agent.status === 'paused' && 'bg-yellow-500/10 text-yellow-500'
                      )}
                    >
                      {agent.status}
                    </span>
                  </div>

                  {/* Load */}
                  <div className="flex items-center">
                    <div className="flex items-center gap-2">
                      <div className="w-16 h-2 bg-apex-bg-primary rounded-full overflow-hidden">
                        <div
                          className="h-full bg-blue-500 rounded-full"
                          style={{
                            width: `${(agent.currentLoad / agent.maxLoad) * 100}%`,
                          }}
                        />
                      </div>
                      <span className="text-xs text-apex-text-tertiary">
                        {agent.currentLoad}/{agent.maxLoad}
                      </span>
                    </div>
                  </div>

                  {/* Success Rate */}
                  <div className="flex items-center">
                    <span
                      className={cn(
                        'text-sm font-medium',
                        agent.successRate >= 0.95 && 'text-green-500',
                        agent.successRate >= 0.8 &&
                          agent.successRate < 0.95 &&
                          'text-yellow-500',
                        agent.successRate < 0.8 && 'text-red-500'
                      )}
                    >
                      {(agent.successRate * 100).toFixed(1)}%
                    </span>
                  </div>

                  {/* Cost */}
                  <div className="flex items-center">
                    <span className="text-sm font-mono text-apex-text-secondary">
                      {formatCost(agent.totalCost)}
                    </span>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-end gap-1">
                    <button
                      className="p-1.5 hover:bg-apex-bg-primary rounded-md transition-colors"
                      onClick={(e) => {
                        e.stopPropagation()
                        if (agent.status === 'paused') {
                          handleResume(agent.id)
                        } else {
                          handlePause(agent.id)
                        }
                      }}
                    >
                      {agent.status === 'paused' ? (
                        <Play size={16} className="text-green-500" />
                      ) : (
                        <Pause size={16} className="text-yellow-500" />
                      )}
                    </button>
                    <button
                      className="p-1.5 hover:bg-apex-bg-primary rounded-md transition-colors"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleAgentClick(agent)
                      }}
                    >
                      <MoreVertical size={16} className="text-apex-text-tertiary" />
                    </button>
                  </div>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </div>
      )}

      {/* Intervention Panel */}
      <AnimatePresence>
        {selectedAgent && (
          <InterventionPanel
            agent={selectedAgent}
            onClose={() => setSelectedAgent(null)}
            onSendMessage={(agentId, message) => {
              console.log(`[Nudge] Agent ${agentId}: ${message}`)
              toast.success('Nudge sent')
            }}
            onPause={handlePause}
            onResume={handleResume}
            onPatchState={async (agentId, state) => {
              try {
                const parsed = JSON.parse(state) as Record<string, unknown>
                await agentApi.update(agentId, parsed)
                toast.success('Agent state patched')
                fetchAgents()
              } catch {
                toast.error('Failed to patch agent state')
              }
            }}
            onTakeover={(agentId) => {
              console.log(`[Takeover] Agent ${agentId}`)
              toast.success('Takeover initiated')
            }}
            onKill={handleKill}
          />
        )}
      </AnimatePresence>
    </div>
  )
}
