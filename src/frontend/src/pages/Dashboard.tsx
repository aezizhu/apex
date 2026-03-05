import { useEffect, useCallback, useMemo, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
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
  Send,
  ChevronDown,
  Terminal,
  Loader2,
  Sparkles,
} from 'lucide-react'
import toast from 'react-hot-toast'
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

      for (const node of nodes) {
        node.x += node.vx
        node.y += node.vy
        node.pulse += node.pulseSpeed

        if (node.x < -20) node.x = w + 20
        if (node.x > w + 20) node.x = -20
        if (node.y < -20) node.y = h + 20
        if (node.y > h + 20) node.y = -20
      }

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

      for (const node of nodes) {
        const pulseAlpha = 0.3 + Math.sin(node.pulse) * 0.15
        const pulseRadius = node.radius + Math.sin(node.pulse) * 0.8

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
// Stat Readout — compact metric display
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
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Icon size={14} className="opacity-50" color={color} />
            <span className="text-[11px] font-mono text-apex-text-tertiary uppercase tracking-wider">
              {label}
            </span>
          </div>
        </div>
        <div className="stat-value text-2xl font-semibold tracking-tight text-apex-text-primary">
          {value}
        </div>
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
// Mission Command Input
// ═══════════════════════════════════════════════════════════════════

const PRIORITY_OPTIONS = [
  { value: 5, label: 'Normal', color: 'text-apex-text-tertiary' },
  { value: 8, label: 'High', color: 'text-amber-400' },
  { value: 10, label: 'Critical', color: 'text-red-400' },
] as const

function MissionCommandInput({ onMissionLaunched }: { onMissionLaunched: () => void }) {
  const [objective, setObjective] = useState('')
  const [priority, setPriority] = useState(5)
  const [showPriority, setShowPriority] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const priorityRef = useRef<HTMLDivElement>(null)

  const selectedPriority = PRIORITY_OPTIONS.find(p => p.value === priority) || PRIORITY_OPTIONS[0]

  // Close priority dropdown on outside click
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (priorityRef.current && !priorityRef.current.contains(e.target as HTMLElement)) {
        setShowPriority(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  const handleSubmit = async () => {
    const trimmed = objective.trim()
    if (!trimmed || isSubmitting) return

    setIsSubmitting(true)
    try {
      await taskApi.create({
        name: trimmed.length > 60 ? trimmed.slice(0, 60) + '...' : trimmed,
        instruction: trimmed,
        priority,
      })
      toast.success('Mission dispatched to the swarm')
      setObjective('')
      setPriority(5)
      onMissionLaunched()
    } catch {
      toast.error('Failed to dispatch mission — check backend connection')
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      handleSubmit()
    }
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: -12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.1, duration: 0.5, ease: [0.25, 0.1, 0.25, 1] }}
    >
      <div className="relative rounded-xl border border-apex-border-subtle/50 bg-apex-bg-secondary/60 backdrop-blur-md overflow-hidden
                      focus-within:border-blue-500/40 transition-colors duration-300">
        {/* Accent glow on top */}
        <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-blue-500/50 to-transparent" />

        {/* Header bar */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-b border-apex-border-subtle/30">
          <Terminal size={14} className="text-blue-400/70" />
          <span className="text-[11px] font-mono text-apex-text-tertiary uppercase tracking-wider">
            Mission Command
          </span>
          <div className="flex-1" />
          <span className="text-[10px] font-mono text-apex-text-muted">
            {navigator.platform.includes('Mac') ? '\u2318' : 'Ctrl'}+Enter to launch
          </span>
        </div>

        {/* Input area */}
        <div className="p-4">
          <textarea
            ref={textareaRef}
            value={objective}
            onChange={(e) => setObjective(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Describe your mission objective... The swarm will decompose, plan, and execute with parallel agents."
            className="w-full bg-transparent text-[14px] text-apex-text-primary placeholder:text-apex-text-muted/50
                       resize-none outline-none min-h-[72px] leading-relaxed"
            rows={3}
            disabled={isSubmitting}
          />
        </div>

        {/* Action bar */}
        <div className="flex items-center justify-between px-4 py-3 border-t border-apex-border-subtle/20 bg-apex-bg-tertiary/30">
          <div className="flex items-center gap-3">
            {/* Priority selector */}
            <div ref={priorityRef} className="relative">
              <button
                onClick={() => setShowPriority(!showPriority)}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-mono
                           bg-white/[0.03] border border-apex-border-subtle/30 hover:border-apex-border-subtle/50
                           transition-colors"
              >
                <span className="text-apex-text-muted">Priority:</span>
                <span className={selectedPriority.color}>{selectedPriority.label}</span>
                <ChevronDown size={12} className="text-apex-text-muted" />
              </button>

              <AnimatePresence>
                {showPriority && (
                  <motion.div
                    initial={{ opacity: 0, y: -4 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -4 }}
                    transition={{ duration: 0.15 }}
                    className="absolute bottom-full left-0 mb-1 py-1 rounded-lg border border-apex-border-subtle/50
                               bg-apex-bg-secondary shadow-xl min-w-[120px] z-50"
                  >
                    {PRIORITY_OPTIONS.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => { setPriority(opt.value); setShowPriority(false) }}
                        className={cn(
                          'w-full text-left px-3 py-1.5 text-[11px] font-mono hover:bg-white/[0.04] transition-colors',
                          priority === opt.value ? 'bg-white/[0.06]' : ''
                        )}
                      >
                        <span className={opt.color}>{opt.label}</span>
                      </button>
                    ))}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>

            <div className="text-[10px] text-apex-text-muted font-mono flex items-center gap-1.5">
              <Sparkles size={11} className="text-purple-400/50" />
              Agents will bid, plan, and execute in parallel
            </div>
          </div>

          <button
            onClick={handleSubmit}
            disabled={!objective.trim() || isSubmitting}
            className={cn(
              'flex items-center gap-2 px-4 py-2 rounded-lg text-[12px] font-medium transition-all duration-200',
              objective.trim() && !isSubmitting
                ? 'bg-blue-500 hover:bg-blue-400 text-white shadow-lg shadow-blue-500/20'
                : 'bg-white/[0.04] text-apex-text-muted cursor-not-allowed'
            )}
          >
            {isSubmitting ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <Send size={14} />
            )}
            Launch
          </button>
        </div>
      </div>
    </motion.div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════════════════

const STATUS_CONFIG: Record<string, { color: string; pulse: boolean; label: string; icon: LucideIcon }> = {
  pending: { color: 'bg-amber-400/60', pulse: false, label: 'Queued', icon: Clock },
  ready: { color: 'bg-amber-400', pulse: true, label: 'Ready', icon: Zap },
  running: { color: 'bg-blue-400', pulse: true, label: 'Running', icon: Loader2 },
  completed: { color: 'bg-emerald-400', pulse: false, label: 'Done', icon: CheckCircle },
  failed: { color: 'bg-red-400', pulse: false, label: 'Failed', icon: AlertTriangle },
  cancelled: { color: 'bg-gray-500', pulse: false, label: 'Cancelled', icon: AlertTriangle },
}

function TaskStatusBadge({ status }: { status: string }) {
  const fallback = { color: 'bg-gray-500', pulse: false, label: status, icon: Clock }
  const config = STATUS_CONFIG[status] || fallback
  const Icon = config.icon
  return (
    <div className="flex items-center gap-1.5">
      <div className={cn('w-1.5 h-1.5 rounded-full shrink-0', config.color, config.pulse && 'animate-pulse')} />
      {status === 'running' ? (
        <Icon size={11} className="text-blue-400 animate-spin" />
      ) : null}
      <span className="text-[10px] font-mono text-apex-text-muted uppercase">{config.label}</span>
    </div>
  )
}

function timeAgo(dateStr: string): string {
  const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000)
  if (seconds < 5) return 'just now'
  if (seconds < 60) return `${seconds}s ago`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  return `${Math.floor(seconds / 86400)}d ago`
}

export default function Dashboard() {
  const metrics = useStore((s) => s.metrics)
  const agents = useStore(selectAgentList)
  const tasks = useStore(selectTaskList)
  const wsConnected = useStore((s) => s.wsConnected)
  const setAgents = useStore((s) => s.setAgents)
  const setTasks = useStore((s) => s.setTasks)
  const setMetrics = useStore((s) => s.setMetrics)
  const [missionActive, setMissionActive] = useState(false)
  const pollRef = useRef<NodeJS.Timeout>()

  // FIX Issue #5: Remove duplicate data fetching - useInitialData already fetches this data
  // Only fetch on-demand when user explicitly needs fresh data (e.g., after launching a mission)
  const refreshData = useCallback(async () => {
    try {
      const [agentsRes, tasksRes, metricsRes] = await Promise.allSettled([
        agentApi.listRaw(),
        taskApi.list({ pageSize: 200 }),
        metricsApi.getSystem(),
      ])
      let hasError = false
      if (agentsRes.status === 'fulfilled' && Array.isArray(agentsRes.value.data)) {
        setAgents(agentsRes.value.data)
      } else {
        hasError = true
      }
      if (tasksRes.status === 'fulfilled' && Array.isArray(tasksRes.value.data)) {
        setTasks(tasksRes.value.data)
      } else {
        hasError = true
      }
      if (metricsRes.status === 'fulfilled' && metricsRes.value.data) {
        setMetrics(metricsRes.value.data)
      } else {
        hasError = true
      }
      // FIX Issue #6: Show toast notification for refresh errors
      if (hasError) {
        toast.error('Failed to refresh data')
      }
    } catch {
      // FIX Issue #6: Show toast notification for errors
      toast.error('Failed to refresh data')
    }
  }, [setAgents, setTasks, setMetrics])

  // FIX Issue #5: Removed initial fetch - useInitialData in App.tsx handles this
  // Only poll when a mission is active (backup for WebSocket)
  useEffect(() => {
    if (missionActive) {
      pollRef.current = setInterval(refreshData, 3000)
    }
    return () => {
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [missionActive, refreshData])

  // Auto-detect active missions and stop polling when all done
  const activeTasks = useMemo(
    () => tasks.filter((t) => t.status === 'running' || t.status === 'pending' || t.status === 'ready'),
    [tasks]
  )

  useEffect(() => {
    if (missionActive && activeTasks.length === 0 && tasks.length > 0) {
      // All tasks finished — keep polling a bit then stop
      const timeout = setTimeout(() => setMissionActive(false), 6000)
      return () => clearTimeout(timeout)
    }
  }, [missionActive, activeTasks.length, tasks.length])

  const handleMissionLaunched = useCallback(() => {
    setMissionActive(true)
    refreshData()
  }, [refreshData])

  const recentTasks = useMemo(
    () =>
      [...tasks]
        .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
        .slice(0, 12),
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
      <div className="relative z-10 p-6 space-y-5 max-w-[1600px] mx-auto">
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
              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-white/[0.04] border border-apex-border-subtle/30">
                <div className={cn(
                  'w-1.5 h-1.5 rounded-full',
                  wsConnected ? 'bg-cyan-400' : 'bg-red-400 animate-pulse'
                )} />
                <span className="text-[10px] font-mono text-apex-text-tertiary uppercase tracking-widest">
                  {wsConnected ? 'Live' : 'Offline'}
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

        {/* ═══ Mission Command Input ═══ */}
        <MissionCommandInput onMissionLaunched={handleMissionLaunched} />

        {/* Stat Readouts — 6-column instrumentation strip */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
          <StatReadout
            label="Agents"
            value={metrics.totalAgents}
            sublabel={busyAgents > 0 ? `${busyAgents} active` : 'none active'}
            icon={Users}
            color="#3b82f6"
            delay={0.15}
          />
          <StatReadout
            label="Tasks"
            value={metrics.runningTasks}
            sublabel={`${metrics.totalTasks} total`}
            icon={Layers}
            color="#8b5cf6"
            delay={0.2}
          />
          <StatReadout
            label="Completed"
            value={completedToday}
            sublabel="today"
            icon={CheckCircle}
            color="#10b981"
            delay={0.25}
          />
          <StatReadout
            label="Success"
            value={`${(metrics.successRate * 100).toFixed(1)}%`}
            sublabel={metrics.failedTasks > 0 ? `${metrics.failedTasks} failed` : 'no failures'}
            icon={Activity}
            color={metrics.successRate >= 0.95 ? '#10b981' : metrics.successRate >= 0.8 ? '#f59e0b' : '#ef4444'}
            delay={0.3}
          />
          <StatReadout
            label="Tokens"
            value={formatTokens(metrics.totalTokens)}
            sublabel="consumed"
            icon={Zap}
            color="#f59e0b"
            delay={0.35}
          />
          <StatReadout
            label="Cost"
            value={formatCost(metrics.totalCost)}
            sublabel="total spend"
            icon={DollarSign}
            color="#06b6d4"
            delay={0.4}
          />
        </div>

        {/* Main Grid: Swarm Visualization + Sidebar */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-4">

          {/* Agent Swarm Visualization */}
          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.45, duration: 0.4 }}
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
            <div className="h-[360px] relative">
              {hasAgents ? (
                <AgentGrid maxAgents={200} />
              ) : (
                <div className="absolute inset-0 flex flex-col items-center justify-center gap-4">
                  <div className="relative">
                    <div className="w-20 h-20 rounded-2xl border border-dashed border-apex-border-subtle/50 flex items-center justify-center animate-float">
                      <Cpu size={28} className="text-apex-text-muted/40" />
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
                    <p className="text-sm text-apex-text-tertiary font-medium">No agents registered</p>
                    <p className="text-xs text-apex-text-muted mt-1">
                      Register agents to see the swarm topology
                    </p>
                  </div>
                </div>
              )}
            </div>
          </motion.div>

          {/* Right Column — System Health + Recent Tasks */}
          <div className="lg:col-span-4 space-y-4">

            {/* System Health */}
            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.5, duration: 0.4 }}
              className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm"
            >
              <div className="flex items-center gap-2 px-4 py-3 border-b border-apex-border-subtle/30">
                <Activity size={14} className="text-emerald-400/60" />
                <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
                  SYSTEM HEALTH
                </span>
              </div>
              <div className="p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Clock size={13} className="text-apex-text-muted" />
                    <span className="text-[12px] text-apex-text-tertiary">Avg Latency</span>
                  </div>
                  <span className="stat-value text-[13px] text-apex-text-secondary">
                    {formatDuration(metrics.avgLatencyMs)}
                  </span>
                </div>

                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ArrowUpRight size={13} className="text-apex-text-muted" />
                    <span className="text-[12px] text-apex-text-tertiary">Throughput</span>
                  </div>
                  <span className="stat-value text-[13px] text-apex-text-secondary">
                    {metrics.completedTasks} completed
                  </span>
                </div>

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

                <div className="pt-2 mt-1 border-t border-apex-border-subtle/20">
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-[11px] text-apex-text-muted font-mono">CAPACITY</span>
                    <span className="text-[11px] text-apex-text-tertiary font-mono">
                      {metrics.activeAgents}/{metrics.totalAgents || '\u2014'}
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

            {/* Live Mission Feed */}
            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.55, duration: 0.4 }}
              className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm flex-1"
            >
              <div className="flex items-center justify-between px-4 py-3 border-b border-apex-border-subtle/30">
                <div className="flex items-center gap-2">
                  <Layers size={14} className="text-purple-400/60" />
                  <span className="text-[12px] font-mono text-apex-text-secondary font-medium">
                    MISSION FEED
                  </span>
                </div>
                {missionActive && (
                  <motion.div
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    className="flex items-center gap-1.5"
                  >
                    <div className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
                    <span className="text-[10px] font-mono text-blue-400">LIVE</span>
                  </motion.div>
                )}
              </div>

              {/* Active missions summary bar */}
              {activeTasks.length > 0 && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  className="px-4 py-2.5 border-b border-apex-border-subtle/20 bg-blue-500/[0.04]"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Loader2 size={12} className="text-blue-400 animate-spin" />
                      <span className="text-[11px] text-blue-300 font-mono">
                        {activeTasks.filter(t => t.status === 'running').length} running
                        {activeTasks.filter(t => t.status === 'pending' || t.status === 'ready').length > 0 &&
                          ` · ${activeTasks.filter(t => t.status === 'pending' || t.status === 'ready').length} queued`}
                      </span>
                    </div>
                    <span className="text-[10px] font-mono text-apex-text-muted">
                      {tasks.filter(t => t.status === 'completed').length}/{tasks.length} done
                    </span>
                  </div>
                  {/* Progress bar */}
                  <div className="mt-2 h-1 bg-apex-bg-tertiary rounded-full overflow-hidden">
                    <motion.div
                      className="h-full rounded-full"
                      style={{ background: 'linear-gradient(90deg, #3b82f6, #8b5cf6)' }}
                      initial={{ width: '0%' }}
                      animate={{
                        width: tasks.length > 0
                          ? `${(tasks.filter(t => t.status === 'completed').length / tasks.length) * 100}%`
                          : '0%'
                      }}
                      transition={{ duration: 0.5, ease: 'easeOut' }}
                    />
                  </div>
                </motion.div>
              )}

              <div className="p-2 max-h-[400px] overflow-y-auto">
                {recentTasks.length === 0 ? (
                  <div className="py-8 text-center">
                    <p className="text-[12px] text-apex-text-muted">No missions dispatched yet</p>
                    <p className="text-[11px] text-apex-text-muted/60 mt-1">
                      Launch a mission above to begin orchestration
                    </p>
                  </div>
                ) : (
                  <div className="space-y-0.5">
                    <AnimatePresence mode="popLayout">
                      {recentTasks.map((task) => (
                        <motion.div
                          key={task.id}
                          layout
                          initial={{ opacity: 0, x: -12, height: 0 }}
                          animate={{ opacity: 1, x: 0, height: 'auto' }}
                          exit={{ opacity: 0, x: 12, height: 0 }}
                          transition={{ duration: 0.25, ease: [0.25, 0.1, 0.25, 1] }}
                          className={cn(
                            'flex items-center gap-3 px-2.5 py-2.5 rounded-lg transition-colors',
                            task.status === 'running' && 'bg-blue-500/[0.05] border border-blue-500/10',
                            task.status !== 'running' && 'hover:bg-white/[0.02]',
                          )}
                        >
                          <TaskStatusBadge status={task.status} />
                          <div className="flex-1 min-w-0">
                            <div className="text-[12px] text-apex-text-secondary truncate">
                              {task.name}
                            </div>
                            {task.agentId && (
                              <div className="text-[10px] text-apex-text-muted font-mono truncate mt-0.5">
                                agent: {task.agentId.slice(0, 8)}
                              </div>
                            )}
                          </div>
                          <div className="text-right shrink-0">
                            <div className="text-[10px] font-mono text-apex-text-muted">
                              {task.costDollars > 0 ? formatCost(task.costDollars) : timeAgo(task.createdAt)}
                            </div>
                            {task.tokensUsed > 0 && (
                              <div className="text-[9px] font-mono text-apex-text-muted/60">
                                {formatTokens(task.tokensUsed)}
                              </div>
                            )}
                          </div>
                        </motion.div>
                      ))}
                    </AnimatePresence>
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
          transition={{ delay: 0.6, duration: 0.4 }}
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
