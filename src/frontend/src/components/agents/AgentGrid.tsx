import { useMemo, useCallback, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { useStore, selectAgentList, Agent } from '../../lib/store'
import { cn, getConfidenceColor, formatCost, formatTokens } from '../../lib/utils'

interface AgentGridProps {
  onAgentSelect?: (agent: Agent) => void
  maxAgents?: number
}

// Hexagon SVG path
const HEX_PATH = 'M50 0 L93.3 25 L93.3 75 L50 100 L6.7 75 L6.7 25 Z'

export default function AgentGrid({ onAgentSelect, maxAgents = 500 }: AgentGridProps) {
  const agents = useStore(selectAgentList)
  const selectedAgentId = useStore((s) => s.selectedAgentId)
  const setSelectedAgentId = useStore((s) => s.setSelectedAgentId)
  const [hoveredAgent, setHoveredAgent] = useState<Agent | null>(null)

  // Limit agents for performance
  const displayedAgents = useMemo(
    () => agents.slice(0, maxAgents),
    [agents, maxAgents]
  )

  // Calculate grid dimensions
  const gridSize = useMemo(() => {
    const count = displayedAgents.length
    const cols = Math.ceil(Math.sqrt(count * 1.5))
    const rows = Math.ceil(count / cols)
    return { cols, rows }
  }, [displayedAgents.length])

  const handleAgentClick = useCallback(
    (agent: Agent) => {
      setSelectedAgentId(agent.id === selectedAgentId ? null : agent.id)
      onAgentSelect?.(agent)
    },
    [selectedAgentId, setSelectedAgentId, onAgentSelect]
  )

  const getStatusColor = (status: Agent['status']): string => {
    switch (status) {
      case 'busy':
        return '#3b82f6' // Blue
      case 'idle':
        return '#6b7280' // Gray
      case 'error':
        return '#ef4444' // Red
      case 'paused':
        return '#f59e0b' // Amber
      default:
        return '#6b7280'
    }
  }

  return (
    <div className="relative w-full h-full overflow-auto">
      {/* Grid Container */}
      <div
        className="relative"
        style={{
          width: gridSize.cols * 70 + 50,
          height: gridSize.rows * 60 + 50,
          minHeight: 400,
        }}
      >
        {displayedAgents.map((agent, index) => {
          const col = index % gridSize.cols
          const row = Math.floor(index / gridSize.cols)
          const offset = row % 2 === 1 ? 35 : 0
          const x = col * 70 + offset + 25
          const y = row * 60 + 25

          const isSelected = agent.id === selectedAgentId
          const confidence = agent.confidence ?? agent.successRate

          return (
            <motion.div
              key={agent.id}
              initial={{ opacity: 0, scale: 0 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0 }}
              transition={{ delay: index * 0.001 }}
              className="absolute cursor-pointer"
              style={{ left: x, top: y }}
              onClick={() => handleAgentClick(agent)}
              onMouseEnter={() => setHoveredAgent(agent)}
              onMouseLeave={() => setHoveredAgent(null)}
            >
              <svg
                width="60"
                height="60"
                viewBox="0 0 100 100"
                className={cn(
                  'transition-all duration-200',
                  isSelected && 'drop-shadow-[0_0_10px_rgba(59,130,246,0.5)]'
                )}
              >
                {/* Background hexagon */}
                <path
                  d={HEX_PATH}
                  fill={getConfidenceColor(confidence)}
                  fillOpacity={0.2}
                  stroke={getStatusColor(agent.status)}
                  strokeWidth={isSelected ? 4 : 2}
                  className="transition-all duration-200"
                />
                {/* Inner glow for busy agents */}
                {agent.status === 'busy' && (
                  <path
                    d={HEX_PATH}
                    fill="none"
                    stroke="#3b82f6"
                    strokeWidth={2}
                    className="animate-pulse-glow"
                    style={{ filter: 'blur(2px)' }}
                  />
                )}
                {/* Status indicator dot */}
                <circle
                  cx="50"
                  cy="50"
                  r="8"
                  fill={getStatusColor(agent.status)}
                />
                {/* Load indicator (arc) */}
                {agent.currentLoad > 0 && (
                  <circle
                    cx="50"
                    cy="50"
                    r="20"
                    fill="none"
                    stroke="#3b82f6"
                    strokeWidth={3}
                    strokeDasharray={`${(agent.currentLoad / agent.maxLoad) * 125.6} 125.6`}
                    strokeLinecap="round"
                    transform="rotate(-90 50 50)"
                  />
                )}
              </svg>
            </motion.div>
          )
        })}
      </div>

      {/* Hover Card */}
      <AnimatePresence>
        {hoveredAgent && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            className="fixed z-50 glass rounded-lg p-4 shadow-xl pointer-events-none"
            style={{
              left: 'calc(50% - 150px)',
              bottom: 100,
              width: 300,
            }}
          >
            <div className="flex items-center justify-between mb-2">
              <span className="font-semibold">{hoveredAgent.name}</span>
              <span
                className={cn(
                  'px-2 py-0.5 rounded text-xs font-medium',
                  hoveredAgent.status === 'busy' && 'bg-blue-500/20 text-blue-400',
                  hoveredAgent.status === 'idle' && 'bg-gray-500/20 text-gray-400',
                  hoveredAgent.status === 'error' && 'bg-red-500/20 text-red-400',
                  hoveredAgent.status === 'paused' && 'bg-yellow-500/20 text-yellow-400'
                )}
              >
                {hoveredAgent.status}
              </span>
            </div>
            <div className="space-y-1 text-sm text-apex-text-secondary">
              <div className="flex justify-between">
                <span>Model:</span>
                <span className="text-apex-text-primary font-mono">
                  {hoveredAgent.model}
                </span>
              </div>
              <div className="flex justify-between">
                <span>Load:</span>
                <span className="text-apex-text-primary">
                  {hoveredAgent.currentLoad}/{hoveredAgent.maxLoad}
                </span>
              </div>
              <div className="flex justify-between">
                <span>Success Rate:</span>
                <span
                  className={cn(
                    'font-medium',
                    hoveredAgent.successRate >= 0.95 && 'text-green-500',
                    hoveredAgent.successRate >= 0.8 &&
                      hoveredAgent.successRate < 0.95 &&
                      'text-yellow-500',
                    hoveredAgent.successRate < 0.8 && 'text-red-500'
                  )}
                >
                  {(hoveredAgent.successRate * 100).toFixed(1)}%
                </span>
              </div>
              <div className="flex justify-between">
                <span>Tokens:</span>
                <span className="text-apex-text-primary font-mono">
                  {formatTokens(hoveredAgent.totalTokens)}
                </span>
              </div>
              <div className="flex justify-between">
                <span>Cost:</span>
                <span className="text-apex-text-primary font-mono">
                  {formatCost(hoveredAgent.totalCost)}
                </span>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Legend */}
      <div className="absolute bottom-4 right-4 glass rounded-lg p-3">
        <div className="text-xs font-medium mb-2 text-apex-text-secondary">
          Agent Status
        </div>
        <div className="space-y-1">
          {[
            { status: 'busy', label: 'Busy', color: '#3b82f6' },
            { status: 'idle', label: 'Idle', color: '#6b7280' },
            { status: 'error', label: 'Error', color: '#ef4444' },
            { status: 'paused', label: 'Paused', color: '#f59e0b' },
          ].map(({ status, label, color }) => (
            <div key={status} className="flex items-center gap-2 text-xs">
              <div
                className="w-3 h-3 rounded-full"
                style={{ backgroundColor: color }}
              />
              <span className="text-apex-text-secondary">{label}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Agent Count */}
      <div className="absolute top-4 left-4 text-sm text-apex-text-secondary">
        {displayedAgents.length} agents
        {agents.length > maxAgents && ` (showing ${maxAgents})`}
      </div>
    </div>
  )
}
