import { useEffect, useCallback, useMemo, useRef } from 'react'
import { motion } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import {
  Activity,
  Users,
  DollarSign,
  Zap,
  Clock,
  CheckCircle,
  AlertTriangle,
  Cpu,
  Network,
  ArrowUpRight,
  Layers,
} from 'lucide-react'
import { useStore, selectAgentList, selectTaskList } from '../lib/store'
import { cn, formatCost, formatTokens, formatDuration } from '../lib/utils'
import { agentApi, taskApi, metricsApi } from '../lib/api'
import AgentGrid from '../components/agents/AgentGrid'
import MetricsChart from '../components/metrics/MetricsChart'

// ═══════════════════════════════════════════════════════════════════
// Neural Constellation — ambient background visualization
// ═══════════════════════════════════════════════════════════════════

interface Node {
  x: number
  y: number
  vx: number
  vy: number
  radius: number
  pulse: number
  pulseSpeed: number
}

function NeuralConstellation({ agentCount }: { agentCount: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const nodesRef = useRef<Node[]>([])
  const animRef = useRef<number>(0)
  const timeRef = useRef(0)

  // Stable node count: either real agents or ghost nodes
  const nodeCount = Math.max(agentCount, 12)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const resize = () => {
      const dpr = window.devicePixelRatio || 1
      const rect = canvas.getBoundingClientRect()
      canvas.width = rect.width * dpr
      canvas.height = rect.height * dpr
      ctx.scale(dpr, dpr)
    }

    resize()
    window.addEventListener('resize', resize)

    // Initialize nodes
    const w = canvas.getBoundingClientRect().width
    const h = canvas.getBoundingClientRect().height

    if (nodesRef.current.length !== nodeCount) {
      nodesRef.current = Array.from({ length: nodeCount }, () => ({
        x: Math.random() * w,
        y: Math.random() * h,
        vx: (Math.random() - 0.5) * 0.3,
        vy: (Math.random() - 0.5) * 0.3,
        radius: 2 + Math.random() * 2,
        pulse: Math.random() * Math.PI * 2,
        pulseSpeed: 0.01 + Math.random() * 0.02,
      }))
    }

    const draw = () => {
      const rect = canvas.getBoundingClientRect()
      const w = rect.width
      const h = rect.height

      ctx.clearRect(0, 0, w, h)
      timeRef.current += 0.016

      const nodes = nodesRef.current

      // Update positions
      for (const node of nodes) {
        node.x += node.vx
        node.y += node.vy
        node.pulse += node.pulseSpeed

        // Wrap around edges
        if (node.x < -20) node.x = w + 20
        if (node.x > w + 20) node.x = -20
        if (node.y < -20) node.y = h + 20
        if (node.y > h + 20) node.y = -20
      }

      // Draw connections
      const connectionDist = 160
      for (let i = 0; i < nodes.length; i++) {
        const nodeA = nodes[i]!
        for (let j = i + 1; j < nodes.length; j++) {
          const nodeB = nodes[j]!
          const dx = nodeA.x - nodeB.x
          const dy = nodeA.y - nodeB.y
          const dist = Math.sqrt(dx * dx + dy * dy)

          if (dist < connectionDist) {
            const alpha = (1 - dist / connectionDist) * 0.12
            ctx.beginPath()
            ctx.moveTo(nodeA.x, nodeA.y)
            ctx.lineTo(nodeB.x, nodeB.y)
            ctx.strokeStyle = `rgba(59, 130, 246, ${alpha})`
            ctx.lineWidth = 0.5
            ctx.stroke()
          }
        }
      }

      // Draw nodes
      for (const node of nodes) {
        const pulseAlpha = 0.3 + Math.sin(node.pulse) * 0.15
        const pulseRadius = node.radius + Math.sin(node.pulse) * 0.8

        // Outer glow
        const gradient = ctx.createRadialGradient(
          node.x, node.y, 0,
          node.x, node.y, pulseRadius * 4
        )
        gradient.addColorStop(0, `rgba(59, 130, 246, ${pulseAlpha * 0.3})`)
        gradient.addColorStop(1, 'rgba(59, 130, 246, 0)')
        ctx.beginPath()
        ctx.arc(node.x, node.y, pulseRadius * 4, 0, Math.PI * 2)
        ctx.fillStyle = gradient
        ctx.fill()

        // Core
        ctx.beginPath()
        ctx.arc(node.x, node.y, pulseRadius, 0, Math.PI * 2)
        ctx.fillStyle = agentCount > 0
          ? `rgba(59, 130, 246, ${pulseAlpha + 0.2})`
          : `rgba(100, 116, 139, ${pulseAlpha})`
        ctx.fill()
      }

      animRef.current = requestAnimationFrame(draw)
    }

    animRef.current = requestAnimationFrame(draw)

    return () => {
      window.removeEventListener('resize', resize)
      cancelAnimationFrame(animRef.current)
    }
  }, [nodeCount, agentCount])

  return (
    <canvas
      ref={canvasRef}
      className="absolute inset-0 w-full h-full"
      style={{ opacity: 0.7 }}
    />
  )
}

// ═══════════════════════════════════════════════════════════════════
// Stat Readout — instrumentation-grade metric display
// ═══════════════════════════════════════════════════════════════════

interface StatReadoutProps {
  label: string
  value: string | number
  sublabel?: string
  icon: LucideIcon
  color: string
  delay?: number
}

function StatReadout({ label, value, sublabel, icon: Icon, color, delay = 0 }: StatReadoutProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.4, ease: [0.25, 0.1, 0.25, 1] }}
      className="group relative"
    >
      <div className="p-4 rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/50 backdrop-blur-sm
                      hover:border-apex-border-subtle transition-all duration-300 card-accent-top"
        style={{ '--card-accent': color } as React.CSSProperties}
      >
        {/* Label row */}
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Icon size={14} className="opacity-50" color={color} />
            <span className="text-[11px] font-mono text-apex-text-tertiary uppercase tracking-wider">
              {label}
            </span>
          </div>
        </div>

        {/* Value */}
        <div className="stat-value text-2xl font-semibold tracking-tight text-apex-text-primary">
          {value}
        </div>

        {/* Sublabel */}
        {sublabel && (
          <div className="mt-1 text-[11px] text-apex-text-muted font-mono">
            {sublabel}
          </div>
        )}
      </div>
    </motion.div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════════════════

export default function Dashboard() {
  const metrics = useStore((s) => s.metrics)
  const agents = useStore(selectAgentList)
  const tasks = useStore(selectTaskList)
  const setAgents = useStore((s) => s.setAgents)
  const setTasks = useStore((s) => s.setTasks)
  const setMetrics = useStore((s) => s.setMetrics)

  const refreshData = useCallback(async () => {
    try {
      const [agentsRes, tasksRes, metricsRes] = await Promise.allSettled([
        agentApi.listRaw(),
        taskApi.list({ pageSize: 200 }),
        metricsApi.getSystem(),
      ])
      if (agentsRes.status === 'fulfilled' && Array.isArray(agentsRes.value.data)) {
        setAgents(agentsRes.value.data)
      }
      if (tasksRes.status === 'fulfilled' && Array.isArray(tasksRes.value.data)) {
        setTasks(tasksRes.value.data)
      }
      if (metricsRes.status === 'fulfilled' && metricsRes.value.data) {
        setMetrics(metricsRes.value.data)
      }
    } catch {
      // Silently handle errors; data may still arrive via WebSocket
    }
  }, [setAgents, setTasks, setMetrics])

  useEffect(() => {
    refreshData()
  }, [refreshData])

  const recentTasks = useMemo(
    () =>
      [...tasks]
        .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
        .slice(0, 6),
    [tasks]
  )

  const completedToday = useMemo(
    () =>
      tasks.filter(
        (t) =>
          t.status === 'completed' &&
          new Date(t.completedAt || '').toDateString() === new Date().toDateString()
      ).length,
    [tasks]
  )

  const busyAgents = agents.filter((a) => a.status === 'busy').length
  const errorAgents = agents.filter((a) => a.status === 'error').length
  const hasAgents = agents.length > 0

  return (
    <div className="relative min-h-full">
      {/* Ambient neural constellation background */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <NeuralConstellation agentCount={agents.length} />
        {/* Radial fade at edges */}
        <div className="absolute inset-0"
          style={{
            background: `
              radial-gradient(ellipse 80% 60% at 50% 40%, transparent 0%, #0a0a0f 100%),
              linear-gradient(to bottom, transparent 60%, #0a0a0f 100%)
            `
          }}
        />
      </div>

      {/* Content */}
      <div className="relative z-10 p-6 space-y-6 max-w-[1600px] mx-auto">
        {/* Header */}
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
          className="flex items-end justify-between"
        >
          <div>
            <div className="flex items-center gap-3 mb-1">
              <h1 className="font-display text-2xl font-bold tracking-tight">
                Command Center
              </h1>
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-white/[0.04] border border-apex-border-subtle/30">
                <div className={cn(
                  'w-1.5 h-1.5 rounded-full',
                  hasAgents ? 'bg-emerald-400 animate-pulse' : 'bg-apex-text-muted'
                )} />
                <span className="text-[10px] font-mono text-apex-text-tertiary uppercase tracking-widest">
                  {hasAgents ? 'Operational' : 'Standby'}
                </span>
              </div>
            </div>
            <p className="text-sm text-apex-text-tertiary">
              Multi-agent orchestration &mdash; real-time swarm telemetry
            </p>
          </div>
          <div className="text-[11px] font-mono text-apex-text-muted">
            {new Date().toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' })}
            {' '}
            <span className="text-apex-text-tertiary">
              {new Date().toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })}
            </span>
          </div>
        </motion.div>

        {/* Stat Readouts — 6-column instrumentation strip */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
          <StatReadout
            label="Agents"
            value={metrics.totalAgents}
            sublabel={busyAgents > 0 ? `${busyAgents} active` : 'none active'}
            icon={Users}
            color="#3b82f6"
            delay={0}
          />
          <StatReadout
            label="Tasks"
            value={metrics.runningTasks}
            sublabel={`${metrics.totalTasks} total`}
            icon={Layers}
            color="#8b5cf6"
            delay={0.05}
          />
          <StatReadout
            label="Completed"
            value={completedToday}
            sublabel="today"
            icon={CheckCircle}
            color="#10b981"
            delay={0.1}
          />
          <StatReadout
            label="Success"
            value={`${(metrics.successRate * 100).toFixed(1)}%`}
            sublabel={metrics.failedTasks > 0 ? `${metrics.failedTasks} failed` : 'no failures'}
            icon={Activity}
            color={metrics.successRate >= 0.95 ? '#10b981' : metrics.successRate >= 0.8 ? '#f59e0b' : '#ef4444'}
            delay={0.15}
          />
          <StatReadout
            label="Tokens"
            value={formatTokens(metrics.totalTokens)}
            sublabel="consumed"
            icon={Zap}
            color="#f59e0b"
            delay={0.2}
          />
          <StatReadout
            label="Cost"
            value={formatCost(metrics.totalCost)}
            sublabel="total spend"
            icon={DollarSign}
            color="#06b6d4"
            delay={0.25}
          />
        </div>

        {/* Main Grid: Swarm Visualization + Sidebar */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-4">

          {/* Agent Swarm Visualization */}
          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.3, duration: 0.4 }}
            className="lg:col-span-8 rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm overflow-hidden"
          >
            <div className="flex items-center justify-between px-4 py-3 border-b border-apex-border-subtle/30">
              <div className="flex items-center gap-2">
                <Network size={14} className="text-blue-400/60" />
                <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
                  SWARM TOPOLOGY
                </span>
              </div>
              <div className="flex items-center gap-3">
                {[
                  { label: 'Active', color: '#3b82f6', count: busyAgents },
                  { label: 'Idle', color: '#6b7280', count: agents.filter(a => a.status === 'idle').length },
                  { label: 'Error', color: '#ef4444', count: errorAgents },
                ].map(({ label, color, count }) => (
                  <div key={label} className="flex items-center gap-1.5 text-[10px] font-mono text-apex-text-muted">
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: color, opacity: count > 0 ? 1 : 0.3 }} />
                    <span>{count}</span>
                  </div>
                ))}
              </div>
            </div>
            <div className="h-[420px] relative">
              {hasAgents ? (
                <AgentGrid maxAgents={200} />
              ) : (
                <div className="absolute inset-0 flex flex-col items-center justify-center gap-4">
                  <div className="relative">
                    <div className="w-20 h-20 rounded-2xl border border-dashed border-apex-border-subtle/50 flex items-center justify-center animate-float">
                      <Cpu size={28} className="text-apex-text-muted/40" />
                    </div>
                    {/* Orbiting dots */}
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
                    <p className="text-sm text-apex-text-tertiary font-medium">No agents registered</p>
                    <p className="text-xs text-apex-text-muted mt-1">
                      Register agents to see the swarm topology
                    </p>
                  </div>
                </div>
              )}
            </div>
          </motion.div>

          {/* Right Column — Activity Feed + System Health */}
          <div className="lg:col-span-4 space-y-4">

            {/* System Health */}
            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.35, duration: 0.4 }}
              className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm"
            >
              <div className="flex items-center gap-2 px-4 py-3 border-b border-apex-border-subtle/30">
                <Activity size={14} className="text-emerald-400/60" />
                <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
                  SYSTEM HEALTH
                </span>
              </div>
              <div className="p-4 space-y-3">
                {/* Latency */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Clock size={13} className="text-apex-text-muted" />
                    <span className="text-[12px] text-apex-text-tertiary">Avg Latency</span>
                  </div>
                  <span className="stat-value text-[13px] text-apex-text-secondary">
                    {formatDuration(metrics.avgLatencyMs)}
                  </span>
                </div>

                {/* Throughput */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ArrowUpRight size={13} className="text-apex-text-muted" />
                    <span className="text-[12px] text-apex-text-tertiary">Throughput</span>
                  </div>
                  <span className="stat-value text-[13px] text-apex-text-secondary">
                    {metrics.completedTasks} completed
                  </span>
                </div>

                {/* Error rate */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <AlertTriangle size={13} className="text-apex-text-muted" />
                    <span className="text-[12px] text-apex-text-tertiary">Error Rate</span>
                  </div>
                  <span className={cn(
                    'stat-value text-[13px]',
                    metrics.failedTasks === 0 ? 'text-emerald-400' : 'text-red-400'
                  )}>
                    {metrics.totalTasks > 0
                      ? `${((metrics.failedTasks / metrics.totalTasks) * 100).toFixed(1)}%`
                      : '0.0%'}
                  </span>
                </div>

                {/* Capacity bar */}
                <div className="pt-2 mt-1 border-t border-apex-border-subtle/20">
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-[11px] text-apex-text-muted font-mono">CAPACITY</span>
                    <span className="text-[11px] text-apex-text-tertiary font-mono">
                      {metrics.activeAgents}/{metrics.totalAgents || '—'}
                    </span>
                  </div>
                  <div className="h-1.5 bg-apex-bg-tertiary rounded-full overflow-hidden">
                    <motion.div
                      initial={{ width: 0 }}
                      animate={{
                        width: metrics.totalAgents > 0
                          ? `${(metrics.activeAgents / metrics.totalAgents) * 100}%`
                          : '0%'
                      }}
                      transition={{ delay: 0.6, duration: 0.8, ease: 'easeOut' }}
                      className="h-full rounded-full"
                      style={{
                        background: 'linear-gradient(90deg, #3b82f6, #8b5cf6)',
                      }}
                    />
                  </div>
                </div>
              </div>
            </motion.div>

            {/* Recent Activity */}
            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.4, duration: 0.4 }}
              className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm flex-1"
            >
              <div className="flex items-center gap-2 px-4 py-3 border-b border-apex-border-subtle/30">
                <Layers size={14} className="text-purple-400/60" />
                <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
                  RECENT TASKS
                </span>
              </div>
              <div className="p-2">
                {recentTasks.length === 0 ? (
                  <div className="py-8 text-center">
                    <p className="text-[12px] text-apex-text-muted">No tasks yet</p>
                    <p className="text-[11px] text-apex-text-muted/60 mt-1">
                      Submit a task to begin orchestration
                    </p>
                  </div>
                ) : (
                  <div className="space-y-0.5">
                    {recentTasks.map((task) => (
                      <div
                        key={task.id}
                        className="flex items-center gap-3 px-2.5 py-2 rounded-lg hover:bg-white/[0.02] transition-colors"
                      >
                        <div className={cn(
                          'w-1.5 h-1.5 rounded-full shrink-0',
                          task.status === 'completed' && 'bg-emerald-400',
                          task.status === 'running' && 'bg-blue-400 animate-pulse',
                          task.status === 'failed' && 'bg-red-400',
                          task.status === 'pending' && 'bg-amber-400/60',
                          !['completed', 'running', 'failed', 'pending'].includes(task.status) && 'bg-gray-500',
                        )} />
                        <div className="flex-1 min-w-0">
                          <div className="text-[12px] text-apex-text-secondary truncate">
                            {task.name}
                          </div>
                        </div>
                        <span className="text-[10px] font-mono text-apex-text-muted shrink-0">
                          {formatCost(task.costDollars)}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </motion.div>
          </div>
        </div>

        {/* Performance Trends */}
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.45, duration: 0.4 }}
          className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm"
        >
          <div className="flex items-center gap-2 px-4 py-3 border-b border-apex-border-subtle/30">
            <Activity size={14} className="text-emerald-400/60" />
            <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
              PERFORMANCE TRENDS
            </span>
          </div>
          <div className="h-[260px] p-2">
            <MetricsChart />
          </div>
        </motion.div>
      </div>
    </div>
  )
}
