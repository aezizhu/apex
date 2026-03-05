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

const getStatusColor = (status: Agent['status']): string => {
  switch (status) {
    case 'busy':
      return '#3b82f6'
    case 'idle':
      return '#6b7280'
    case 'error':
      return '#ef4444'
    case 'paused':
      return '#f59e0b'
    default:
      return '#6b7280'
  }
}

export default function AgentGrid({ onAgentSelect, maxAgents = 500 }: AgentGridProps) {
  const agents = useStore(selectAgentList)
  const selectedAgentId = useStore((s) => s.selectedAgentId)
  const setSelectedAgentId = useStore((s) => s.setSelectedAgentId)
  const [hoveredAgent, setHoveredAgent] = useState<Agent | null>(null)

  const displayedAgents = useMemo(
    () => agents.slice(0, maxAgents),
    [agents, maxAgents]
  )

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

  // Empty state: ghostly hexagonal lattice
  if (displayedAgents.length === 0) {
    const ghostCols = 8
    const ghostRows = 5
    return (
      <div className="relative w-full h-full flex items-center justify-center overflow-hidden">
        <svg
          className="absolute inset-0 w-full h-full opacity-[0.04]"
          viewBox={`0 0 ${ghostCols * 70 + 50} ${ghostRows * 60 + 50}`}
          preserveAspectRatio="xMidYMid meet"
        >
          {Array.from({ length: ghostCols * ghostRows }).map((_, index) => {
            const col = index % ghostCols
            const row = Math.floor(index / ghostCols)
            const offset = row % 2 === 1 ? 35 : 0
            const x = col * 70 + offset + 25
            const y = row * 60 + 25
            return (
              <g key={index} transform={`translate(${x}, ${y}) scale(0.55)`}>
                <path
                  d={HEX_PATH}
                  fill="none"
                  stroke="#3b82f6"
                  strokeWidth={1}
                />
              </g>
            )
          })}
        </svg>
      </div>
    )
  }

  // Compute node positions for all agents
  const nodePositions = useMemo(() => {
    return displayedAgents.map((_, index) => {
      const col = index % gridSize.cols
      const row = Math.floor(index / gridSize.cols)
      const offset = row % 2 === 1 ? 35 : 0
      const cx = col * 70 + offset + 25 + 30 // center of hexagon
      const cy = row * 60 + 25 + 30
      return { cx, cy, col, row }
    })
  }, [displayedAgents, gridSize])

  // Compute edges between neighboring hexagons
  const edges = useMemo(() => {
    const result: Array<{ x1: number; y1: number; x2: number; y2: number; idx1: number; idx2: number }> = []
    for (let i = 0; i < nodePositions.length; i++) {
      for (let j = i + 1; j < nodePositions.length; j++) {
        const a = nodePositions[i]!
        const b = nodePositions[j]!
        const dx = a.cx - b.cx
        const dy = a.cy - b.cy
        const dist = Math.sqrt(dx * dx + dy * dy)
        // Connect if within ~1.2 hex-widths (adjacent in hex grid)
        if (dist < 90) {
          result.push({ x1: a.cx, y1: a.cy, x2: b.cx, y2: b.cy, idx1: i, idx2: j })
        }
      }
    }
    return result
  }, [nodePositions])

  const svgWidth = gridSize.cols * 70 + 100
  const svgHeight = gridSize.rows * 60 + 100

  return (
    <div className="relative w-full h-full overflow-auto">
      {/* SVG for connection lines */}
      <svg
        className="absolute inset-0 pointer-events-none"
        width={svgWidth}
        height={svgHeight}
        style={{ minHeight: 400 }}
      >
        <defs>
          <linearGradient id="edge-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#3b82f6" stopOpacity="0.3" />
            <stop offset="50%" stopColor="#8b5cf6" stopOpacity="0.15" />
            <stop offset="100%" stopColor="#3b82f6" stopOpacity="0.3" />
          </linearGradient>
          <filter id="edge-glow">
            <feGaussianBlur stdDeviation="1.5" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        {edges.map((edge, i) => {
          const agent1 = displayedAgents[edge.idx1]
          const agent2 = displayedAgents[edge.idx2]
          const eitherBusy = agent1?.status === 'busy' || agent2?.status === 'busy'
          return (
            <line
              key={i}
              x1={edge.x1}
              y1={edge.y1}
              x2={edge.x2}
              y2={edge.y2}
              stroke={eitherBusy ? '#3b82f6' : '#6b7280'}
              strokeWidth={eitherBusy ? 1.5 : 0.8}
              strokeOpacity={eitherBusy ? 0.5 : 0.15}
              filter={eitherBusy ? 'url(#edge-glow)' : undefined}
            />
          )
        })}
        {/* Animated data flow particles on edges for busy agents */}
        {edges.map((edge, i) => {
          const agent1 = displayedAgents[edge.idx1]
          const agent2 = displayedAgents[edge.idx2]
          if (agent1?.status !== 'busy' && agent2?.status !== 'busy') return null
          return (
            <circle key={`pulse-${i}`} r="2" fill="#3b82f6" opacity="0.7">
              <animateMotion
                dur={`${2 + Math.random() * 2}s`}
                repeatCount="indefinite"
                path={`M${edge.x1},${edge.y1} L${edge.x2},${edge.y2}`}
              />
            </circle>
          )
        })}
      </svg>

      {/* Grid Container */}
      <div
        className="relative"
        style={{
          width: svgWidth,
          height: svgHeight,
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
          const statusColor = getStatusColor(agent.status)

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
                  isSelected && 'drop-shadow-[0_0_12px_rgba(59,130,246,0.5)]'
                )}
              >
                {/* Background hexagon */}
                <path
                  d={HEX_PATH}
                  fill={getConfidenceColor(confidence)}
                  fillOpacity={0.15}
                  stroke={statusColor}
                  strokeWidth={isSelected ? 3 : 1.5}
                  strokeOpacity={isSelected ? 1 : 0.6}
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
                  r="6"
                  fill={statusColor}
                  opacity={0.9}
                />
                {/* Agent name label */}
                <text
                  x="50"
                  y="85"
                  textAnchor="middle"
                  fontSize="9"
                  fill={statusColor}
                  fillOpacity={0.7}
                  fontFamily="monospace"
                >
                  {agent.name.length > 10 ? agent.name.slice(0, 9) + '...' : agent.name}
                </text>
                {/* Load indicator (arc) */}
                {agent.currentLoad > 0 && (
                  <circle
                    cx="50"
                    cy="50"
                    r="18"
                    fill="none"
                    stroke={statusColor}
                    strokeWidth={2}
                    strokeOpacity={0.4}
                    strokeDasharray={`${(agent.currentLoad / agent.maxLoad) * 113} 113`}
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
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 6 }}
            transition={{ duration: 0.15 }}
            className="fixed z-50 glass-strong rounded-lg p-3.5 shadow-2xl pointer-events-none"
            style={{
              left: 'calc(50% - 140px)',
              bottom: 80,
              width: 280,
            }}
          >
            <div className="flex items-center justify-between mb-2.5">
              <span className="text-[13px] font-semibold">{hoveredAgent.name}</span>
              <span
                className={cn(
                  'px-2 py-0.5 rounded text-[10px] font-mono font-medium',
                  hoveredAgent.status === 'busy' && 'bg-blue-500/15 text-blue-400',
                  hoveredAgent.status === 'idle' && 'bg-gray-500/15 text-gray-400',
                  hoveredAgent.status === 'error' && 'bg-red-500/15 text-red-400',
                  hoveredAgent.status === 'paused' && 'bg-yellow-500/15 text-yellow-400'
                )}
              >
                {hoveredAgent.status}
              </span>
            </div>
            <div className="space-y-1.5 text-[12px]">
              <div className="flex justify-between">
                <span className="text-apex-text-muted">Model</span>
                <span className="text-apex-text-secondary font-mono text-[11px]">
                  {hoveredAgent.model}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-apex-text-muted">Load</span>
                <span className="text-apex-text-secondary font-mono text-[11px]">
                  {hoveredAgent.currentLoad}/{hoveredAgent.maxLoad}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-apex-text-muted">Success</span>
                <span
                  className={cn(
                    'font-mono text-[11px]',
                    hoveredAgent.successRate >= 0.95 && 'text-emerald-400',
                    hoveredAgent.successRate >= 0.8 &&
                      hoveredAgent.successRate < 0.95 &&
                      'text-amber-400',
                    hoveredAgent.successRate < 0.8 && 'text-red-400'
                  )}
                >
                  {(hoveredAgent.successRate * 100).toFixed(1)}%
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-apex-text-muted">Tokens</span>
                <span className="text-apex-text-secondary font-mono text-[11px]">
                  {formatTokens(hoveredAgent.totalTokens)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-apex-text-muted">Cost</span>
                <span className="text-apex-text-secondary font-mono text-[11px]">
                  {formatCost(hoveredAgent.totalCost)}
                </span>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Agent Count — top right */}
      <div className="absolute top-3 right-3 text-[10px] font-mono text-apex-text-muted bg-apex-bg-primary/60 backdrop-blur-sm px-2 py-1 rounded">
        {displayedAgents.length} agents
        {agents.length > maxAgents && ` (${maxAgents} shown)`}
      </div>
    </div>
  )
}
