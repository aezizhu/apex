import { useState, useMemo, useCallback, useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  X,
  Clock,
  Cpu,
  DollarSign,
  Terminal,
  Maximize2,
  Activity,
} from 'lucide-react'
import { Card } from '@/components/ui/Card'
import { cn, formatCost, formatTokens } from '@/lib/utils'

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

export type AgentStatus = 'idle' | 'busy' | 'error' | 'paused'

export interface AgentScreen {
  agentId: string
  agentName: string
  status: AgentStatus
  currentAction: string
  lastToolCall?: { name: string; params: Record<string, unknown>; result?: string }
  screenContent: string
  focusArea?: string
  tokensUsed: number
  costSoFar: number
  startedAt: string
}

interface AgentSightProps {
  agents: AgentScreen[]
  zoomLevel?: 1 | 2 | 4
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

const statusDotColor: Record<AgentStatus, string> = {
  idle: 'bg-gray-400',
  busy: 'bg-blue-500',
  error: 'bg-red-500',
  paused: 'bg-yellow-500',
}

const statusPulse: Record<AgentStatus, boolean> = {
  idle: false,
  busy: true,
  error: false,
  paused: false,
}

function elapsed(startedAt: string): string {
  const diff = Date.now() - new Date(startedAt).getTime()
  if (diff < 0) return '0s'
  const secs = Math.floor(diff / 1000)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m ${secs % 60}s`
  const hrs = Math.floor(mins / 60)
  return `${hrs}h ${mins % 60}m`
}

function formatParams(params: Record<string, unknown>): string {
  try {
    return JSON.stringify(params, null, 2)
  } catch {
    return '{}'
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Typing Indicator
// ═══════════════════════════════════════════════════════════════════════════════

function TypingIndicator() {
  return (
    <span className="inline-flex items-center gap-0.5 ml-1">
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          className="w-1 h-1 rounded-full bg-apex-accent-primary"
          animate={{ opacity: [0.3, 1, 0.3] }}
          transition={{ duration: 1, repeat: Infinity, delay: i * 0.2 }}
        />
      ))}
    </span>
  )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Progress Bar
// ═══════════════════════════════════════════════════════════════════════════════

function ContractProgressBar({ tokensUsed, maxTokens = 1_000_000 }: { tokensUsed: number; maxTokens?: number }) {
  const pct = Math.min((tokensUsed / maxTokens) * 100, 100)
  return (
    <div className="w-full h-1.5 bg-apex-bg-primary rounded-full overflow-hidden">
      <motion.div
        className={cn(
          'h-full rounded-full',
          pct < 50 ? 'bg-blue-500' : pct < 80 ? 'bg-yellow-500' : 'bg-red-500'
        )}
        initial={{ width: 0 }}
        animate={{ width: `${pct}%` }}
        transition={{ duration: 0.5 }}
      />
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Agent Card
// ═══════════════════════════════════════════════════════════════════════════════

function AgentScreenCard({
  agent,
  onExpand,
}: {
  agent: AgentScreen
  onExpand: (agent: AgentScreen) => void
}) {
  const [elapsedStr, setElapsedStr] = useState(() => elapsed(agent.startedAt))

  useEffect(() => {
    const interval = setInterval(() => {
      setElapsedStr(elapsed(agent.startedAt))
    }, 1000)
    return () => clearInterval(interval)
  }, [agent.startedAt])

  return (
    <Card
      variant="glass"
      padding="none"
      interactive
      className="flex flex-col overflow-hidden group"
      onClick={() => onExpand(agent)}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-apex-border-subtle">
        <div className="flex items-center gap-2 min-w-0">
          <span className="relative flex h-2.5 w-2.5 shrink-0">
            <span
              className={cn(
                'absolute inline-flex h-full w-full rounded-full opacity-75',
                statusDotColor[agent.status],
                statusPulse[agent.status] && 'animate-ping'
              )}
            />
            <span
              className={cn(
                'relative inline-flex rounded-full h-2.5 w-2.5',
                statusDotColor[agent.status]
              )}
            />
          </span>
          <span className="text-sm font-medium truncate">{agent.agentName}</span>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation()
            onExpand(agent)
          }}
          className="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-apex-bg-tertiary transition-all"
        >
          <Maximize2 size={14} className="text-apex-text-tertiary" />
        </button>
      </div>

      {/* Current Action */}
      <div className="px-3 py-2 bg-apex-bg-tertiary/50">
        <div className="flex items-center gap-1.5 text-xs text-apex-text-secondary">
          <Activity size={12} className="text-apex-accent-primary shrink-0" />
          <span className="truncate">{agent.currentAction}</span>
          {agent.status === 'busy' && <TypingIndicator />}
        </div>
      </div>

      {/* Tool Call */}
      {agent.lastToolCall && (
        <div className="px-3 py-2 border-b border-apex-border-subtle">
          <div className="flex items-center gap-1.5 mb-1">
            <Terminal size={11} className="text-apex-text-tertiary shrink-0" />
            <span className="text-[11px] font-mono text-apex-accent-primary">
              {agent.lastToolCall.name}
            </span>
          </div>
          <pre className="text-[10px] font-mono text-apex-text-tertiary leading-tight max-h-12 overflow-hidden">
            {formatParams(agent.lastToolCall.params)}
          </pre>
        </div>
      )}

      {/* Screen Content */}
      <div className="px-3 py-2 flex-1 min-h-0 overflow-hidden">
        <p className="text-[11px] text-apex-text-secondary leading-relaxed line-clamp-3">
          {agent.screenContent}
        </p>
        {agent.focusArea && (
          <span className="inline-block mt-1 px-1.5 py-0.5 text-[10px] bg-apex-accent-primary/10 text-apex-accent-primary rounded">
            {agent.focusArea}
          </span>
        )}
      </div>

      {/* Footer Stats */}
      <div className="px-3 py-2 border-t border-apex-border-subtle">
        <ContractProgressBar tokensUsed={agent.tokensUsed} />
        <div className="flex items-center justify-between mt-1.5 text-[10px] text-apex-text-tertiary">
          <div className="flex items-center gap-1">
            <Cpu size={10} />
            <span>{formatTokens(agent.tokensUsed)}</span>
          </div>
          <div className="flex items-center gap-1">
            <DollarSign size={10} />
            <span>{formatCost(agent.costSoFar)}</span>
          </div>
          <div className="flex items-center gap-1">
            <Clock size={10} />
            <span>{elapsedStr}</span>
          </div>
        </div>
      </div>
    </Card>
  )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Expanded Modal
// ═══════════════════════════════════════════════════════════════════════════════

function ExpandedAgentModal({
  agent,
  onClose,
}: {
  agent: AgentScreen
  onClose: () => void
}) {
  const [elapsedStr, setElapsedStr] = useState(() => elapsed(agent.startedAt))

  useEffect(() => {
    const interval = setInterval(() => {
      setElapsedStr(elapsed(agent.startedAt))
    }, 1000)
    return () => clearInterval(interval)
  }, [agent.startedAt])

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  return (
    <motion.div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />

      {/* Modal */}
      <motion.div
        className="relative w-full max-w-3xl max-h-[90vh] bg-apex-bg-secondary border border-apex-border-subtle rounded-xl overflow-hidden flex flex-col"
        initial={{ scale: 0.95, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.95, y: 20 }}
      >
        {/* Modal Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-apex-border-subtle">
          <div className="flex items-center gap-3">
            <span className="relative flex h-3 w-3">
              <span
                className={cn(
                  'absolute inline-flex h-full w-full rounded-full opacity-75',
                  statusDotColor[agent.status],
                  statusPulse[agent.status] && 'animate-ping'
                )}
              />
              <span
                className={cn(
                  'relative inline-flex rounded-full h-3 w-3',
                  statusDotColor[agent.status]
                )}
              />
            </span>
            <div>
              <h2 className="text-lg font-semibold">{agent.agentName}</h2>
              <p className="text-xs text-apex-text-tertiary font-mono">{agent.agentId}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-apex-bg-tertiary rounded-lg transition-colors"
          >
            <X size={20} className="text-apex-text-secondary" />
          </button>
        </div>

        {/* Modal Body */}
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {/* Current Action */}
          <div>
            <h3 className="text-sm font-medium text-apex-text-secondary mb-1">Current Action</h3>
            <div className="flex items-center gap-2 text-apex-text-primary">
              <Activity size={16} className="text-apex-accent-primary" />
              <span>{agent.currentAction}</span>
              {agent.status === 'busy' && <TypingIndicator />}
            </div>
          </div>

          {/* Last Tool Call */}
          {agent.lastToolCall && (
            <div>
              <h3 className="text-sm font-medium text-apex-text-secondary mb-1">Last Tool Call</h3>
              <div className="bg-apex-bg-primary rounded-lg p-4 font-mono text-sm">
                <div className="text-apex-accent-primary mb-2">{agent.lastToolCall.name}()</div>
                <pre className="text-apex-text-secondary text-xs overflow-x-auto">
                  {formatParams(agent.lastToolCall.params)}
                </pre>
                {agent.lastToolCall.result && (
                  <div className="mt-3 pt-3 border-t border-apex-border-subtle">
                    <div className="text-green-500 text-xs mb-1">Result:</div>
                    <pre className="text-apex-text-secondary text-xs overflow-x-auto whitespace-pre-wrap">
                      {agent.lastToolCall.result}
                    </pre>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Screen Content */}
          <div>
            <h3 className="text-sm font-medium text-apex-text-secondary mb-1">Working Context</h3>
            <div className="bg-apex-bg-primary rounded-lg p-4">
              <p className="text-sm text-apex-text-secondary whitespace-pre-wrap leading-relaxed">
                {agent.screenContent}
              </p>
              {agent.focusArea && (
                <div className="mt-3 pt-3 border-t border-apex-border-subtle">
                  <span className="text-xs text-apex-text-tertiary">Focus Area:</span>
                  <span className="ml-2 text-sm text-apex-accent-primary">{agent.focusArea}</span>
                </div>
              )}
            </div>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-apex-bg-primary rounded-lg p-3 text-center">
              <div className="text-xs text-apex-text-tertiary mb-1">Tokens Used</div>
              <div className="text-lg font-semibold">{formatTokens(agent.tokensUsed)}</div>
            </div>
            <div className="bg-apex-bg-primary rounded-lg p-3 text-center">
              <div className="text-xs text-apex-text-tertiary mb-1">Cost So Far</div>
              <div className="text-lg font-semibold">{formatCost(agent.costSoFar)}</div>
            </div>
            <div className="bg-apex-bg-primary rounded-lg p-3 text-center">
              <div className="text-xs text-apex-text-tertiary mb-1">Time Elapsed</div>
              <div className="text-lg font-semibold">{elapsedStr}</div>
            </div>
          </div>

          {/* Contract Progress */}
          <div>
            <div className="flex items-center justify-between text-xs text-apex-text-tertiary mb-1">
              <span>Contract Usage</span>
              <span>{formatTokens(agent.tokensUsed)} / 1M tokens</span>
            </div>
            <ContractProgressBar tokensUsed={agent.tokensUsed} />
          </div>
        </div>
      </motion.div>
    </motion.div>
  )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Component
// ═══════════════════════════════════════════════════════════════════════════════

export default function AgentSight({ agents, zoomLevel = 1 }: AgentSightProps) {
  const [expandedAgent, setExpandedAgent] = useState<AgentScreen | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const gridCols = useMemo(() => {
    switch (zoomLevel) {
      case 1:
        return 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4'
      case 2:
        return 'grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6'
      case 4:
        return 'grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8'
      default:
        return 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4'
    }
  }, [zoomLevel])

  const handleExpand = useCallback((agent: AgentScreen) => {
    setExpandedAgent(agent)
  }, [])

  const handleClose = useCallback(() => {
    setExpandedAgent(null)
  }, [])

  // Keep expanded modal in sync with agent updates
  const activeExpandedAgent = useMemo(() => {
    if (!expandedAgent) return null
    return agents.find((a) => a.agentId === expandedAgent.agentId) ?? expandedAgent
  }, [agents, expandedAgent])

  return (
    <>
      <div
        ref={containerRef}
        className={cn('grid gap-4 overflow-auto', gridCols)}
        style={{ maxHeight: 'calc(100vh - 280px)' }}
      >
        <AnimatePresence mode="popLayout">
          {agents.map((agent) => (
            <motion.div
              key={agent.agentId}
              layout
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.2 }}
            >
              <AgentScreenCard agent={agent} onExpand={handleExpand} />
            </motion.div>
          ))}
        </AnimatePresence>
      </div>

      {/* Expanded Modal */}
      <AnimatePresence>
        {activeExpandedAgent && (
          <ExpandedAgentModal agent={activeExpandedAgent} onClose={handleClose} />
        )}
      </AnimatePresence>
    </>
  )
}
