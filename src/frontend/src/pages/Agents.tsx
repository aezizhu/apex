import { useState, useEffect, useCallback, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import {
  Plus,
  Search,
  Grid,
  List,
  MoreVertical,
  Play,
  Pause,
  X,
  Loader2,
  Users,
  Cpu,
  AlertTriangle,
  Zap,
  Activity,
  Shield,
} from 'lucide-react'
import { useStore, selectAgentList, type Agent } from '../lib/store'
import { cn, formatCost } from '../lib/utils'
import { agentApi } from '../lib/api'
import type { DelegateRequest } from '../types'
import AgentGrid from '../components/agents/AgentGrid'
import { InterventionPanel } from '../components/InterventionPanel'
import toast from 'react-hot-toast'

// ═══════════════════════════════════════════════════════════════════
// Fleet Status Readout
// ═══════════════════════════════════════════════════════════════════

function FleetReadout({
  icon: Icon,
  label,
  value,
  color,
  pulse,
}: {
  icon: LucideIcon
  label: string
  value: number
  color: string
  pulse?: boolean
}) {
  return (
    <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg border border-apex-border-subtle/30 bg-apex-bg-secondary/50 backdrop-blur-sm">
      <div className="relative">
        <Icon size={16} className="opacity-50" style={{ color }} />
        {pulse && value > 0 && (
          <span className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full animate-pulse" style={{ backgroundColor: color }} />
        )}
      </div>
      <div className="flex items-baseline gap-2">
        <span className="stat-value text-lg font-semibold text-apex-text-primary">{value}</span>
        <span className="text-[10px] font-mono text-apex-text-muted uppercase tracking-wider">{label}</span>
      </div>
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Status Filter Bar
// ═══════════════════════════════════════════════════════════════════

const STATUS_CONFIG = {
  all: { label: 'All', color: '#94a3b8', dotColor: 'bg-gray-400' },
  idle: { label: 'Standby', color: '#6b7280', dotColor: 'bg-gray-400' },
  busy: { label: 'Active', color: '#3b82f6', dotColor: 'bg-blue-500' },
  error: { label: 'Fault', color: '#ef4444', dotColor: 'bg-red-500' },
  paused: { label: 'Held', color: '#f59e0b', dotColor: 'bg-amber-500' },
} as const

type ViewMode = 'grid' | 'list'

// ═══════════════════════════════════════════════════════════════════
// Main Component
// ═══════════════════════════════════════════════════════════════════

export default function Agents() {
  const agents = useStore(selectAgentList)
  const setAgents = useStore((s) => s.setAgents)
  const [viewMode, setViewMode] = useState<ViewMode>('grid')
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<Agent['status'] | 'all'>('all')
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(null)
  const [showRegister, setShowRegister] = useState(false)
  const [registering, setRegistering] = useState(false)
  const [newAgent, setNewAgent] = useState({
    name: '',
    model: 'claude-sonnet-4-5',
    maxLoad: 10,
  })

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

  const handleRegister = useCallback(async () => {
    if (!newAgent.name.trim()) {
      toast.error('Agent name is required')
      return
    }
    setRegistering(true)
    try {
      await agentApi.create({
        config: {
          name: newAgent.name.trim(),
          model: newAgent.model,
          maxLoad: newAgent.maxLoad,
        },
      })
      toast.success(`Agent "${newAgent.name}" registered`)
      setShowRegister(false)
      setNewAgent({ name: '', model: 'claude-sonnet-4-5', maxLoad: 10 })
      fetchAgents()
    } catch {
      toast.error('Failed to register agent')
    } finally {
      setRegistering(false)
    }
  }, [newAgent, fetchAgents])

  const handleDelegate = useCallback(async (agentId: string, data: { task: string; strategy: string; priority: string; toAgent: string }) => {
    try {
      const req: DelegateRequest = {
        task: data.task,
        strategy: data.strategy as DelegateRequest['strategy'],
        priority: data.priority as DelegateRequest['priority'],
      }
      if (data.strategy === 'direct' && data.toAgent) {
        req.toAgent = data.toAgent
      }
      const response = await agentApi.delegate(agentId, req)
      const result = response.data
      if (result.status === 'accepted') {
        toast.success(`Task delegated to agent ${result.assignedAgent?.slice(0, 8) ?? 'unknown'}`)
      } else {
        toast.error(`Delegation failed: ${result.reason}`)
      }
    } catch {
      toast.error('Failed to delegate task')
    }
  }, [])

  const filteredAgents = agents.filter((agent) => {
    const matchesSearch = agent.name.toLowerCase().includes(searchQuery.toLowerCase())
    const matchesStatus = statusFilter === 'all' || agent.status === statusFilter
    return matchesSearch && matchesStatus
  })

  const statusCounts = useMemo(() => ({
    all: agents.length,
    idle: agents.filter((a) => a.status === 'idle').length,
    busy: agents.filter((a) => a.status === 'busy').length,
    error: agents.filter((a) => a.status === 'error').length,
    paused: agents.filter((a) => a.status === 'paused').length,
  }), [agents])

  const handleAgentClick = (agent: Agent) => {
    setSelectedAgent(selectedAgent?.id === agent.id ? null : agent)
  }

  return (
    <div className="space-y-5">
      {/* ── Header ─────────────────────────────────────────────────── */}
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-blue-500/20 to-purple-500/20 border border-blue-500/20">
              <Shield size={20} className="text-blue-400" />
            </div>
            <div>
              <h1 className="text-xl font-display font-bold tracking-tight">Fleet Control</h1>
              <p className="text-xs font-mono text-apex-text-muted uppercase tracking-wider mt-0.5">
                Agent Swarm Management
              </p>
            </div>
          </div>
        </div>
        <button
          onClick={() => setShowRegister(true)}
          className="flex items-center gap-2 px-3.5 py-2 text-sm font-medium bg-blue-500/10 hover:bg-blue-500/20 text-blue-400 border border-blue-500/20 rounded-lg transition-all hover:border-blue-500/30"
        >
          <Plus size={16} />
          Deploy Agent
        </button>
      </div>

      {/* ── Fleet Status Readouts ────────────────────────────────── */}
      <div className="flex flex-wrap items-center gap-3">
        <FleetReadout icon={Users} label="Total" value={statusCounts.all} color="#94a3b8" />
        <FleetReadout icon={Zap} label="Active" value={statusCounts.busy} color="#3b82f6" pulse />
        <FleetReadout icon={Cpu} label="Standby" value={statusCounts.idle} color="#6b7280" />
        <FleetReadout icon={AlertTriangle} label="Fault" value={statusCounts.error} color="#ef4444" pulse />
        <FleetReadout icon={Pause} label="Held" value={statusCounts.paused} color="#f59e0b" />
      </div>

      {/* ── Controls ──────────────────────────────────────────────── */}
      <div className="flex flex-wrap items-center gap-3">
        {/* Search */}
        <div className="relative flex-1 min-w-[200px] max-w-sm">
          <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-apex-text-muted" />
          <input
            type="text"
            placeholder="Search fleet..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-9 pr-4 py-2 text-sm bg-apex-bg-secondary/80 border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-blue-500/50 font-mono placeholder:font-sans placeholder:text-apex-text-muted transition-colors"
          />
        </div>

        {/* Status Filter */}
        <div className="flex items-center gap-1 p-1 bg-apex-bg-secondary/50 rounded-lg border border-apex-border-subtle/30">
          {(Object.entries(STATUS_CONFIG) as [Agent['status'] | 'all', typeof STATUS_CONFIG['all']][]).map(([key, cfg]) => (
            <button
              key={key}
              onClick={() => setStatusFilter(key)}
              className={cn(
                'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-mono transition-all',
                statusFilter === key
                  ? 'bg-white/[0.06] text-white shadow-sm'
                  : 'text-apex-text-muted hover:text-apex-text-secondary hover:bg-white/[0.02]'
              )}
            >
              {key !== 'all' && (
                <span className={cn('w-1.5 h-1.5 rounded-full', cfg.dotColor, statusFilter === key && 'animate-pulse')} />
              )}
              {cfg.label}
              <span className="text-apex-text-muted/60">{statusCounts[key as keyof typeof statusCounts]}</span>
            </button>
          ))}
        </div>

        {/* View Toggle */}
        <div className="flex items-center gap-0.5 p-1 bg-apex-bg-secondary/50 rounded-lg border border-apex-border-subtle/30">
          <button
            onClick={() => setViewMode('grid')}
            className={cn(
              'p-1.5 rounded-md transition-all',
              viewMode === 'grid'
                ? 'bg-white/[0.06] text-white'
                : 'text-apex-text-muted hover:text-apex-text-secondary'
            )}
          >
            <Grid size={16} />
          </button>
          <button
            onClick={() => setViewMode('list')}
            className={cn(
              'p-1.5 rounded-md transition-all',
              viewMode === 'list'
                ? 'bg-white/[0.06] text-white'
                : 'text-apex-text-muted hover:text-apex-text-secondary'
            )}
          >
            <List size={16} />
          </button>
        </div>
      </div>

      {/* ── Content ───────────────────────────────────────────────── */}
      {viewMode === 'grid' ? (
        <div className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm p-4 h-[600px] relative overflow-hidden">
          {/* Subtle grid background */}
          <div className="absolute inset-0 neural-grid opacity-30 pointer-events-none" />
          <div className="relative h-full">
            {agents.length > 0 ? (
              <AgentGrid maxAgents={500} onAgentSelect={handleAgentClick} />
            ) : (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-4">
                <div className="relative">
                  <div className="w-20 h-20 rounded-2xl border border-dashed border-apex-border-subtle/50 flex items-center justify-center animate-float">
                    <Users size={28} className="text-apex-text-muted/40" />
                  </div>
                  {[0, 1, 2].map((i) => (
                    <div
                      key={i}
                      className="absolute top-1/2 left-1/2 w-1.5 h-1.5 bg-blue-400/30 rounded-full"
                      style={{
                        animation: `orbit ${6 + i * 2}s linear infinite`,
                        animationDelay: `${i * -2}s`,
                        ['--orbit-radius' as string]: `${40 + i * 12}px`,
                      }}
                    />
                  ))}
                </div>
                <div className="text-center">
                  <p className="text-sm text-apex-text-tertiary font-medium">No agents deployed</p>
                  <p className="text-xs text-apex-text-muted mt-1">Deploy an agent to initialize the swarm</p>
                </div>
              </div>
            )}
          </div>
        </div>
      ) : (
        <div className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm overflow-hidden">
          {/* Table Header */}
          <div className="grid grid-cols-[1fr_110px_90px_110px_90px_90px_70px] gap-4 px-5 py-2.5 text-[10px] font-mono text-apex-text-muted uppercase tracking-wider border-b border-apex-border-subtle/40 bg-apex-bg-tertiary/30">
            <div>Designation</div>
            <div>Model</div>
            <div>Status</div>
            <div>Load</div>
            <div>Success</div>
            <div>Cost</div>
            <div></div>
          </div>

          {/* Table Body */}
          <div className="divide-y divide-apex-border-subtle/30">
            <AnimatePresence>
              {filteredAgents.map((agent, i) => (
                <motion.div
                  key={agent.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ delay: i * 0.02 }}
                  className={cn(
                    'grid grid-cols-[1fr_110px_90px_110px_90px_90px_70px] gap-4 px-5 py-3 hover:bg-white/[0.02] transition-all cursor-pointer',
                    selectedAgent?.id === agent.id && 'bg-blue-500/[0.04] border-l-2 border-l-blue-500'
                  )}
                  onClick={() => handleAgentClick(agent)}
                >
                  {/* Name */}
                  <div className="flex items-center gap-3">
                    <div className="relative">
                      <div
                        className={cn(
                          'w-8 h-8 rounded-lg flex items-center justify-center text-xs font-mono font-semibold',
                          agent.status === 'busy' && 'bg-blue-500/15 text-blue-400 border border-blue-500/20',
                          agent.status === 'idle' && 'bg-gray-500/15 text-gray-400 border border-gray-500/20',
                          agent.status === 'error' && 'bg-red-500/15 text-red-400 border border-red-500/20',
                          agent.status === 'paused' && 'bg-amber-500/15 text-amber-400 border border-amber-500/20'
                        )}
                      >
                        {agent.name.charAt(0).toUpperCase()}
                      </div>
                      {agent.status === 'busy' && (
                        <span className="absolute -bottom-0.5 -right-0.5 w-2 h-2 bg-blue-500 rounded-full animate-pulse" />
                      )}
                    </div>
                    <div>
                      <div className="text-sm font-medium">{agent.name}</div>
                      <div className="text-[10px] text-apex-text-muted font-mono">{agent.id.slice(0, 8)}</div>
                    </div>
                  </div>

                  {/* Model */}
                  <div className="flex items-center">
                    <span className="text-xs font-mono text-apex-text-tertiary">{agent.model}</span>
                  </div>

                  {/* Status */}
                  <div className="flex items-center">
                    <span
                      className={cn(
                        'px-2 py-0.5 rounded text-[10px] font-mono uppercase tracking-wider',
                        agent.status === 'busy' && 'bg-blue-500/10 text-blue-400',
                        agent.status === 'idle' && 'bg-gray-500/10 text-gray-400',
                        agent.status === 'error' && 'bg-red-500/10 text-red-400',
                        agent.status === 'paused' && 'bg-amber-500/10 text-amber-400'
                      )}
                    >
                      {agent.status === 'busy' ? 'active' : agent.status === 'idle' ? 'standby' : agent.status}
                    </span>
                  </div>

                  {/* Load */}
                  <div className="flex items-center gap-2">
                    <div className="w-14 h-1.5 bg-apex-bg-primary rounded-full overflow-hidden">
                      <div
                        className="h-full rounded-full transition-all duration-500"
                        style={{
                          width: `${(agent.currentLoad / agent.maxLoad) * 100}%`,
                          background: agent.currentLoad / agent.maxLoad > 0.8
                            ? '#ef4444'
                            : agent.currentLoad / agent.maxLoad > 0.5
                              ? '#f59e0b'
                              : '#3b82f6',
                        }}
                      />
                    </div>
                    <span className="text-[10px] text-apex-text-muted font-mono">
                      {agent.currentLoad}/{agent.maxLoad}
                    </span>
                  </div>

                  {/* Success Rate */}
                  <div className="flex items-center">
                    <span
                      className={cn(
                        'text-xs font-mono',
                        agent.successRate >= 0.95 && 'text-emerald-400',
                        agent.successRate >= 0.8 && agent.successRate < 0.95 && 'text-amber-400',
                        agent.successRate < 0.8 && 'text-red-400'
                      )}
                    >
                      {(agent.successRate * 100).toFixed(1)}%
                    </span>
                  </div>

                  {/* Cost */}
                  <div className="flex items-center">
                    <span className="text-xs font-mono text-apex-text-tertiary">
                      {formatCost(agent.totalCost)}
                    </span>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-end gap-0.5">
                    <button
                      className="p-1 hover:bg-white/[0.04] rounded-md transition-colors"
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
                        <Play size={14} className="text-emerald-400" />
                      ) : (
                        <Pause size={14} className="text-amber-400" />
                      )}
                    </button>
                    <button
                      className="p-1 hover:bg-white/[0.04] rounded-md transition-colors"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleAgentClick(agent)
                      }}
                    >
                      <MoreVertical size={14} className="text-apex-text-muted" />
                    </button>
                  </div>
                </motion.div>
              ))}
            </AnimatePresence>

            {filteredAgents.length === 0 && (
              <div className="flex flex-col items-center justify-center py-16 text-center">
                <Activity size={24} className="text-apex-text-muted/30 mb-3" />
                <p className="text-sm text-apex-text-tertiary">
                  {agents.length === 0 ? 'No agents deployed' : 'No agents match filter'}
                </p>
              </div>
            )}
          </div>
        </div>
      )}

      {/* ── Register Agent Modal ──────────────────────────────────── */}
      <AnimatePresence>
        {showRegister && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
            onClick={() => setShowRegister(false)}
          >
            <motion.div
              initial={{ scale: 0.95, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.95, opacity: 0 }}
              onClick={(e) => e.stopPropagation()}
              className="w-full max-w-md p-6 bg-apex-bg-secondary border border-apex-border-subtle/50 rounded-xl shadow-2xl space-y-5"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="p-1.5 rounded-md bg-blue-500/10 border border-blue-500/20">
                    <Plus size={16} className="text-blue-400" />
                  </div>
                  <div>
                    <h2 className="text-base font-display font-semibold">Deploy Agent</h2>
                    <p className="text-[10px] font-mono text-apex-text-muted uppercase tracking-wider">Fleet Registration</p>
                  </div>
                </div>
                <button
                  onClick={() => setShowRegister(false)}
                  className="p-1 hover:bg-apex-bg-tertiary rounded-md transition-colors"
                >
                  <X size={16} className="text-apex-text-muted" />
                </button>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">Designation</label>
                  <input
                    type="text"
                    value={newAgent.name}
                    onChange={(e) => setNewAgent({ ...newAgent, name: e.target.value })}
                    placeholder="e.g. Research-Alpha"
                    className="w-full px-3 py-2 text-sm bg-apex-bg-primary border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-blue-500/50 font-mono transition-colors"
                    autoFocus
                  />
                </div>

                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">Model</label>
                  <select
                    value={newAgent.model}
                    onChange={(e) => setNewAgent({ ...newAgent, model: e.target.value })}
                    className="w-full px-3 py-2 text-sm bg-apex-bg-primary border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-blue-500/50 font-mono transition-colors"
                  >
                    <option value="claude-opus-4-6">Claude Opus 4.6</option>
                    <option value="claude-sonnet-4-5">Claude Sonnet 4.5</option>
                    <option value="claude-haiku-4-5">Claude Haiku 4.5</option>
                    <option value="gpt-5.2">GPT-5.2</option>
                    <option value="gpt-5.2-mini">GPT-5.2 Mini</option>
                    <option value="o3">o3</option>
                    <option value="o4-mini">o4-mini</option>
                    <option value="gemini-3-pro">Gemini 3 Pro</option>
                    <option value="gemini-3-flash">Gemini 3 Flash</option>
                  </select>
                </div>

                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">Max Concurrent Load</label>
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={newAgent.maxLoad}
                    onChange={(e) => setNewAgent({ ...newAgent, maxLoad: parseInt(e.target.value) || 1 })}
                    className="w-full px-3 py-2 text-sm bg-apex-bg-primary border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-blue-500/50 font-mono transition-colors"
                  />
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-1">
                <button
                  onClick={() => setShowRegister(false)}
                  className="px-3 py-1.5 text-xs font-mono text-apex-text-muted hover:text-apex-text-secondary transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleRegister}
                  disabled={registering || !newAgent.name.trim()}
                  className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-blue-500/10 hover:bg-blue-500/20 text-blue-400 border border-blue-500/20 rounded-lg transition-all disabled:opacity-40"
                >
                  {registering && <Loader2 size={14} className="animate-spin" />}
                  Deploy
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Intervention Panel ─────────────────────────────────────── */}
      <AnimatePresence>
        {selectedAgent && (
          <InterventionPanel
            agent={selectedAgent}
            onClose={() => setSelectedAgent(null)}
            onSendMessage={async (agentId, message) => {
              try {
                await agentApi.update(agentId, { systemPrompt: message })
                toast.success('Nudge sent — agent prompt updated')
                fetchAgents()
              } catch {
                toast.error('Failed to send nudge')
              }
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
            onTakeover={async (agentId) => {
              try {
                await agentApi.pause(agentId)
                toast.success('Agent paused for takeover — manual control active')
                fetchAgents()
              } catch {
                toast.error('Failed to initiate takeover')
              }
            }}
            onKill={handleKill}
            onDelegate={handleDelegate}
          />
        )}
      </AnimatePresence>
    </div>
  )
}
