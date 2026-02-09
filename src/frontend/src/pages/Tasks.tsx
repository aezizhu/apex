import { useState, useEffect, useCallback, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import {
  Search,
  Clock,
  CheckCircle,
  XCircle,
  PlayCircle,
  PauseCircle,
  ChevronDown,
  ChevronRight,
  X,
  Loader2,
  Cpu,
  AlertTriangle,
  RotateCcw,
  Send,
  GitBranch,
  Users,
  Target,
  GanttChart,
  Zap,
  List,
} from 'lucide-react'
import { useStore, selectTaskList, type Task, type Agent } from '../lib/store'
import { cn, formatCost, formatTokens, formatDate, formatDuration } from '../lib/utils'
import { taskApi } from '../lib/api'
import { TaskTimeline } from '../components/TaskTimeline'
import toast from 'react-hot-toast'

// ═══════════════════════════════════════════════════════════════════
// Status Config
// ═══════════════════════════════════════════════════════════════════

const STATUS_ICON: Record<Task['status'], { icon: LucideIcon; color: string; bg: string; label: string }> = {
  completed: { icon: CheckCircle, color: 'text-emerald-400', bg: 'bg-emerald-500/10', label: 'Complete' },
  failed: { icon: XCircle, color: 'text-red-400', bg: 'bg-red-500/10', label: 'Failed' },
  running: { icon: PlayCircle, color: 'text-blue-400', bg: 'bg-blue-500/10', label: 'Running' },
  pending: { icon: Clock, color: 'text-amber-400', bg: 'bg-amber-500/10', label: 'Queued' },
  ready: { icon: Clock, color: 'text-amber-400', bg: 'bg-amber-500/10', label: 'Ready' },
  cancelled: { icon: PauseCircle, color: 'text-gray-400', bg: 'bg-gray-500/10', label: 'Cancelled' },
}

const MISSION_STATUS: Record<string, { label: string; color: string; icon: LucideIcon }> = {
  pending: { label: 'Queued', color: '#f59e0b', icon: Clock },
  running: { label: 'Swarming', color: '#3b82f6', icon: PlayCircle },
  completed: { label: 'Complete', color: '#10b981', icon: CheckCircle },
  failed: { label: 'Failed', color: '#ef4444', icon: XCircle },
}

// ═══════════════════════════════════════════════════════════════════
// Mission Type
// ═══════════════════════════════════════════════════════════════════

interface Mission {
  dagId: string
  name: string
  tasks: Task[]
  status: 'pending' | 'running' | 'completed' | 'failed'
  progress: { total: number; completed: number; running: number; failed: number; pending: number }
  assignedAgents: string[]
  totalCost: number
  totalTokens: number
  createdAt: string
}

function groupIntoMissions(tasks: Task[]): Mission[] {
  const groups = new Map<string, Task[]>()
  for (const task of tasks) {
    const key = task.dagId || task.id
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(task)
  }

  return Array.from(groups.entries())
    .map(([dagId, mTasks]) => {
      const sorted = [...mTasks].sort(
        (a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
      )
      const running = sorted.filter((t) => t.status === 'running').length
      const completed = sorted.filter((t) => t.status === 'completed').length
      const failed = sorted.filter((t) => t.status === 'failed').length
      const pending = sorted.filter(
        (t) => t.status === 'pending' || t.status === 'ready'
      ).length

      let status: Mission['status']
      if (completed === sorted.length) status = 'completed'
      else if (running > 0) status = 'running'
      else if (failed > 0 && pending === 0 && running === 0) status = 'failed'
      else status = 'pending'

      return {
        dagId,
        name: sorted[0].name,
        tasks: sorted,
        status,
        progress: { total: sorted.length, completed, running, failed, pending },
        assignedAgents: [
          ...new Set(sorted.map((t) => t.agentId).filter(Boolean) as string[]),
        ],
        totalCost: sorted.reduce((sum, t) => sum + t.costDollars, 0),
        totalTokens: sorted.reduce((sum, t) => sum + t.tokensUsed, 0),
        createdAt: sorted[0].createdAt,
      }
    })
    .sort((a, b) => {
      const order: Record<string, number> = { running: 0, pending: 1, failed: 2, completed: 3 }
      const diff = (order[a.status] ?? 4) - (order[b.status] ?? 4)
      if (diff !== 0) return diff
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    })
}

// ═══════════════════════════════════════════════════════════════════
// Operation Counter
// ═══════════════════════════════════════════════════════════════════

function OpCounter({
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
    <div
      className="flex-1 min-w-[120px] p-3.5 rounded-xl border border-apex-border-subtle/30 bg-apex-bg-secondary/40 backdrop-blur-sm card-accent-top"
      style={{ ['--card-accent' as string]: color }}
    >
      <div className="flex items-center gap-2 mb-2">
        <Icon size={14} className="opacity-50" style={{ color }} />
        <span className="text-[10px] font-mono text-apex-text-muted uppercase tracking-wider">
          {label}
        </span>
        {pulse && value > 0 && (
          <span
            className="w-1.5 h-1.5 rounded-full animate-pulse ml-auto"
            style={{ backgroundColor: color }}
          />
        )}
      </div>
      <div className="stat-value text-2xl font-semibold text-apex-text-primary">{value}</div>
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Flow Connector (animated line between stages)
// ═══════════════════════════════════════════════════════════════════

function FlowConnector() {
  return (
    <div className="flex items-center self-center">
      <div className="w-8 md:w-14 h-px relative overflow-hidden">
        <div className="absolute inset-0 bg-apex-border-subtle/40" />
        <div
          className="absolute inset-0 animate-shimmer"
          style={{
            background:
              'linear-gradient(90deg, transparent, rgba(139,92,246,0.4), transparent)',
            backgroundSize: '200% 100%',
          }}
        />
      </div>
      <ChevronRight size={10} className="text-apex-text-muted/40 -ml-1" />
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Swarm Flow Diagram (empty state)
// ═══════════════════════════════════════════════════════════════════

const FLOW_STAGES: Array<{
  icon: LucideIcon
  label: string
  desc: string
  color: string
}> = [
  { icon: Send, label: 'DISPATCH', desc: 'Define your objective', color: '#8b5cf6' },
  { icon: GitBranch, label: 'DECOMPOSE', desc: 'System plans sub-tasks', color: '#3b82f6' },
  { icon: Users, label: 'SWARM', desc: 'Agents execute in parallel', color: '#10b981' },
  { icon: Target, label: 'CONVERGE', desc: 'Results aggregate', color: '#f59e0b' },
]

function SwarmFlowDiagram({ onLaunch }: { onLaunch: () => void }) {
  return (
    <div className="flex flex-col items-center py-12 md:py-16 rounded-xl border border-apex-border-subtle/20 bg-apex-bg-secondary/20 neural-grid">
      {/* Title */}
      <motion.div
        initial={{ opacity: 0, y: -10 }}
        animate={{ opacity: 1, y: 0 }}
        className="text-center mb-10"
      >
        <h3 className="text-sm font-mono text-apex-text-muted uppercase tracking-[0.2em]">
          How the Swarm Works
        </h3>
      </motion.div>

      {/* Flow stages */}
      <div className="flex items-start justify-center gap-1 md:gap-2 px-4">
        {FLOW_STAGES.map((stage, i) => (
          <div key={stage.label} className="contents">
            {/* Stage */}
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: i * 0.12, duration: 0.4 }}
              className="flex flex-col items-center gap-2.5 w-[80px] md:w-[100px]"
            >
              {/* Icon box */}
              <div
                className={cn(
                  'w-12 h-12 md:w-14 md:h-14 rounded-xl flex items-center justify-center relative',
                  i === 2 && 'animate-pulse-glow'
                )}
                style={{
                  background: `${stage.color}12`,
                  border: `1px solid ${stage.color}30`,
                  boxShadow: i === 2 ? `0 0 20px ${stage.color}20` : undefined,
                }}
              >
                <stage.icon size={20} style={{ color: stage.color }} />
              </div>

              {/* Parallel agents indicator for SWARM stage */}
              {i === 2 && (
                <div className="flex gap-1.5">
                  {[0, 1, 2].map((j) => (
                    <div
                      key={j}
                      className="w-6 h-6 rounded-md flex items-center justify-center"
                      style={{
                        background: `${stage.color}15`,
                        border: `1px solid ${stage.color}25`,
                      }}
                    >
                      <Cpu
                        size={10}
                        style={{ color: stage.color, opacity: 0.7 }}
                      />
                    </div>
                  ))}
                </div>
              )}

              {/* Label */}
              <div className="text-center">
                <div
                  className="text-[9px] md:text-[10px] font-mono font-semibold tracking-wider"
                  style={{ color: stage.color }}
                >
                  {stage.label}
                </div>
                <div className="text-[9px] md:text-[10px] text-apex-text-muted mt-0.5 leading-tight">
                  {stage.desc}
                </div>
              </div>
            </motion.div>

            {/* Connector between stages */}
            {i < FLOW_STAGES.length - 1 && <FlowConnector />}
          </div>
        ))}
      </div>

      {/* CTA */}
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.6 }}
        className="flex flex-col items-center gap-3 mt-10"
      >
        <button
          onClick={onLaunch}
          className="flex items-center gap-2.5 px-5 py-2.5 text-sm font-medium bg-purple-500/10 hover:bg-purple-500/20 text-purple-400 border border-purple-500/20 rounded-lg transition-all hover:border-purple-500/30 hover:shadow-[0_0_20px_rgba(139,92,246,0.15)]"
        >
          <Zap size={16} />
          Launch Your First Mission
        </button>
        <p className="text-[11px] text-apex-text-muted">
          No active missions — Launch a mission to activate the swarm
        </p>
      </motion.div>
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Mission Card
// ═══════════════════════════════════════════════════════════════════

function MissionCard({
  mission,
  agents,
  expanded,
  onToggle,
  onCancelTask,
  onRetryTask,
}: {
  mission: Mission
  agents: Map<string, Agent>
  expanded: boolean
  onToggle: () => void
  onCancelTask: (id: string) => void
  onRetryTask: (id: string) => void
}) {
  const cfg = MISSION_STATUS[mission.status]
  const StatusIcon = cfg.icon
  const completionPct =
    mission.progress.total > 0
      ? (mission.progress.completed / mission.progress.total) * 100
      : 0

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="rounded-xl border border-apex-border-subtle/30 bg-apex-bg-secondary/30 backdrop-blur-sm overflow-hidden"
    >
      {/* Mission header */}
      <div
        className="flex items-center gap-4 px-4 py-3.5 cursor-pointer hover:bg-white/[0.02] transition-all"
        onClick={onToggle}
      >
        {/* Status icon */}
        <div className="p-1.5 rounded-md" style={{ background: `${cfg.color}15` }}>
          <StatusIcon size={16} style={{ color: cfg.color }} />
        </div>

        {/* Name + meta */}
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">{mission.name}</div>
          <div className="flex items-center gap-2 text-[10px] font-mono text-apex-text-muted mt-0.5">
            <span>{mission.dagId.slice(0, 8)}</span>
            <span className="text-apex-border-subtle">|</span>
            <span>
              {mission.progress.total} {mission.progress.total === 1 ? 'task' : 'tasks'}
            </span>
            {mission.assignedAgents.length > 0 && (
              <>
                <span className="text-apex-border-subtle">|</span>
                <span>
                  {mission.assignedAgents.length}{' '}
                  {mission.assignedAgents.length === 1 ? 'agent' : 'agents'}
                </span>
              </>
            )}
          </div>
        </div>

        {/* Status badge */}
        <span
          className="px-2 py-0.5 rounded text-[10px] font-mono uppercase tracking-wider"
          style={{ background: `${cfg.color}15`, color: cfg.color }}
        >
          {cfg.label}
        </span>

        {/* Progress bar */}
        {mission.progress.total > 1 && (
          <div className="hidden md:flex items-center gap-2 w-28">
            <div className="flex-1 h-1.5 rounded-full bg-apex-bg-tertiary overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-500"
                style={{ width: `${completionPct}%`, background: cfg.color }}
              />
            </div>
            <span className="text-[10px] font-mono text-apex-text-muted whitespace-nowrap">
              {mission.progress.completed}/{mission.progress.total}
            </span>
          </div>
        )}

        {/* Cost */}
        <div className="hidden md:block text-[10px] font-mono text-apex-text-muted">
          {formatCost(mission.totalCost)}
        </div>

        <ChevronDown
          size={14}
          className={cn(
            'text-apex-text-muted transition-transform shrink-0',
            expanded && 'rotate-180'
          )}
        />
      </div>

      {/* Expanded: sub-task grid showing parallel execution */}
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="border-t border-apex-border-subtle/30 overflow-hidden"
          >
            <div className="p-4 space-y-3">
              {/* Active agents banner */}
              {mission.progress.running > 0 && (
                <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-blue-500/5 border border-blue-500/15">
                  <div className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
                  <span className="text-[10px] font-mono text-blue-400 uppercase tracking-wider">
                    {mission.progress.running} {mission.progress.running === 1 ? 'agent' : 'agents'} executing in parallel
                  </span>
                </div>
              )}

              {/* Task grid */}
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                {mission.tasks.map((task) => {
                  const taskCfg = STATUS_ICON[task.status]
                  const TaskIcon = taskCfg.icon
                  const agentName = task.agentId
                    ? agents.get(task.agentId)?.name || task.agentId.slice(0, 8)
                    : null

                  return (
                    <div
                      key={task.id}
                      className={cn(
                        'p-3 rounded-lg bg-apex-bg-primary/50 border border-apex-border-subtle/20 transition-all',
                        task.status === 'running' && 'border-blue-500/20 shadow-[0_0_10px_rgba(59,130,246,0.05)]'
                      )}
                    >
                      <div className="flex items-center gap-2 mb-1.5">
                        <TaskIcon size={12} className={taskCfg.color} />
                        <span
                          className="text-[10px] font-mono uppercase tracking-wider"
                          style={{
                            color:
                              taskCfg.color === 'text-emerald-400'
                                ? '#34d399'
                                : taskCfg.color === 'text-red-400'
                                ? '#f87171'
                                : taskCfg.color === 'text-blue-400'
                                ? '#60a5fa'
                                : taskCfg.color === 'text-amber-400'
                                ? '#fbbf24'
                                : '#9ca3af',
                          }}
                        >
                          {taskCfg.label}
                        </span>
                        {task.status === 'running' && (
                          <div className="w-1 h-1 rounded-full bg-blue-400 animate-pulse ml-auto" />
                        )}
                      </div>
                      <div className="text-xs font-medium truncate">{task.name}</div>
                      <div className="flex items-center gap-2 mt-1.5 text-[10px] font-mono text-apex-text-muted">
                        {agentName && (
                          <span className="flex items-center gap-1">
                            <Cpu size={8} className="opacity-50" />
                            {agentName}
                          </span>
                        )}
                        {task.tokensUsed > 0 && (
                          <span>{formatTokens(task.tokensUsed)} tok</span>
                        )}
                      </div>

                      {/* Task actions */}
                      <div className="flex items-center gap-1.5 mt-2">
                        {task.status === 'running' && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation()
                              onCancelTask(task.id)
                            }}
                            className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono bg-red-500/10 text-red-400 hover:bg-red-500/20 border border-red-500/20 rounded transition-colors"
                          >
                            <XCircle size={10} />
                            Abort
                          </button>
                        )}
                        {task.status === 'failed' && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation()
                              onRetryTask(task.id)
                            }}
                            className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 border border-blue-500/20 rounded transition-colors"
                          >
                            <RotateCcw size={10} />
                            Retry
                          </button>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>

              {/* Mission metrics */}
              <div className="flex items-center gap-4 pt-1 text-[10px] font-mono text-apex-text-muted">
                <span>{formatCost(mission.totalCost)}</span>
                <span className="text-apex-border-subtle">|</span>
                <span>{formatTokens(mission.totalTokens)} tokens</span>
                <span className="text-apex-border-subtle">|</span>
                <span>{formatDate(mission.createdAt)}</span>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}

// ═══════════════════════════════════════════════════════════════════
// Main Component
// ═══════════════════════════════════════════════════════════════════

type ViewMode = 'missions' | 'tasks' | 'timeline'

export default function Tasks() {
  const tasks = useStore(selectTaskList)
  const agents = useStore((s) => s.agents)
  const setTasks = useStore((s) => s.setTasks)
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<Task['status'] | 'all'>('all')
  const [expandedMission, setExpandedMission] = useState<string | null>(null)
  const [viewMode, setViewMode] = useState<ViewMode>('missions')
  const [showCreate, setShowCreate] = useState(false)
  const [creating, setCreating] = useState(false)
  const [newTask, setNewTask] = useState({
    name: '',
    prompt: '',
    priority: 5,
  })

  const fetchTasks = useCallback(async () => {
    try {
      const response = await taskApi.list({ pageSize: 200 })
      if (Array.isArray(response.data)) {
        setTasks(response.data)
      }
    } catch {
      // WebSocket will keep data flowing
    }
  }, [setTasks])

  useEffect(() => {
    fetchTasks()
  }, [fetchTasks])

  const handleCancel = useCallback(
    async (taskId: string) => {
      try {
        await taskApi.cancel(taskId)
        toast.success('Task aborted')
        fetchTasks()
      } catch {
        toast.error('Failed to abort task')
      }
    },
    [fetchTasks]
  )

  const handleRetry = useCallback(
    async (taskId: string) => {
      try {
        await taskApi.retry(taskId)
        toast.success('Task retrying')
        fetchTasks()
      } catch {
        toast.error('Failed to retry task')
      }
    },
    [fetchTasks]
  )

  const handleCreate = useCallback(async () => {
    if (!newTask.name.trim() || !newTask.prompt.trim()) {
      toast.error('Mission name and objective are required')
      return
    }
    setCreating(true)
    try {
      await taskApi.create({
        name: newTask.name.trim(),
        prompt: newTask.prompt.trim(),
        priority: newTask.priority,
      })
      toast.success(`Mission "${newTask.name}" launched`)
      setShowCreate(false)
      setNewTask({ name: '', prompt: '', priority: 5 })
      fetchTasks()
    } catch {
      toast.error('Failed to launch mission')
    } finally {
      setCreating(false)
    }
  }, [newTask, fetchTasks])

  // Filtered tasks
  const filteredTasks = useMemo(
    () =>
      tasks
        .filter((task) => {
          const matchesSearch = task.name
            .toLowerCase()
            .includes(searchQuery.toLowerCase())
          const matchesStatus =
            statusFilter === 'all' || task.status === statusFilter
          return matchesSearch && matchesStatus
        })
        .sort(
          (a, b) =>
            new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        ),
    [tasks, searchQuery, statusFilter]
  )

  // Missions (grouped by DAG)
  const missions = useMemo(
    () => groupIntoMissions(filteredTasks),
    [filteredTasks]
  )

  // Counts (from all tasks, not filtered)
  const counts = useMemo(
    () => ({
      pending: tasks.filter(
        (t) => t.status === 'pending' || t.status === 'ready'
      ).length,
      running: tasks.filter((t) => t.status === 'running').length,
      completed: tasks.filter((t) => t.status === 'completed').length,
      failed: tasks.filter((t) => t.status === 'failed').length,
    }),
    [tasks]
  )

  return (
    <div className="space-y-5 p-5">
      {/* ── Header ─────────────────────────────────────────────────── */}
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-purple-500/20 to-blue-500/20 border border-purple-500/20">
              <Zap size={20} className="text-purple-400" />
            </div>
            <div>
              <h1 className="text-xl font-display font-bold tracking-tight">
                Swarm Operations
              </h1>
              <p className="text-xs font-mono text-apex-text-muted uppercase tracking-wider mt-0.5">
                Mission Orchestration &amp; Parallel Execution
              </p>
            </div>
          </div>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-3.5 py-2 text-sm font-medium bg-purple-500/10 hover:bg-purple-500/20 text-purple-400 border border-purple-500/20 rounded-lg transition-all hover:border-purple-500/30"
        >
          <Zap size={16} />
          Launch Mission
        </button>
      </div>

      {/* ── Operation Counters ────────────────────────────────────── */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <OpCounter
          icon={Clock}
          label="Queued"
          value={counts.pending}
          color="#f59e0b"
        />
        <OpCounter
          icon={Cpu}
          label="Executing"
          value={counts.running}
          color="#3b82f6"
          pulse
        />
        <OpCounter
          icon={CheckCircle}
          label="Complete"
          value={counts.completed}
          color="#10b981"
        />
        <OpCounter
          icon={AlertTriangle}
          label="Failed"
          value={counts.failed}
          color="#ef4444"
          pulse
        />
      </div>

      {/* ── Controls ──────────────────────────────────────────────── */}
      <div className="flex flex-wrap items-center gap-3">
        {/* Search */}
        <div className="relative flex-1 min-w-[200px] max-w-sm">
          <Search
            size={15}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-apex-text-muted"
          />
          <input
            type="text"
            placeholder="Search missions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-9 pr-4 py-2 text-sm bg-apex-bg-secondary/80 border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-purple-500/50 font-mono placeholder:font-sans placeholder:text-apex-text-muted transition-colors"
          />
        </div>

        {/* Status Filter */}
        <select
          value={statusFilter}
          onChange={(e) =>
            setStatusFilter(e.target.value as Task['status'] | 'all')
          }
          className="px-3 py-2 text-xs font-mono bg-apex-bg-secondary/80 border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-purple-500/50 text-apex-text-secondary transition-colors"
        >
          <option value="all">All Status</option>
          <option value="pending">Queued</option>
          <option value="running">Executing</option>
          <option value="completed">Complete</option>
          <option value="failed">Failed</option>
          <option value="cancelled">Cancelled</option>
        </select>

        {/* View Toggle */}
        <div className="flex items-center gap-0.5 p-1 bg-apex-bg-secondary/50 rounded-lg border border-apex-border-subtle/30">
          <button
            onClick={() => setViewMode('missions')}
            className={cn(
              'p-1.5 rounded-md transition-all',
              viewMode === 'missions'
                ? 'bg-white/[0.06] text-white'
                : 'text-apex-text-muted hover:text-apex-text-secondary'
            )}
            title="Missions view"
          >
            <Zap size={16} />
          </button>
          <button
            onClick={() => setViewMode('tasks')}
            className={cn(
              'p-1.5 rounded-md transition-all',
              viewMode === 'tasks'
                ? 'bg-white/[0.06] text-white'
                : 'text-apex-text-muted hover:text-apex-text-secondary'
            )}
            title="Task list view"
          >
            <List size={16} />
          </button>
          <button
            onClick={() => setViewMode('timeline')}
            className={cn(
              'p-1.5 rounded-md transition-all',
              viewMode === 'timeline'
                ? 'bg-white/[0.06] text-white'
                : 'text-apex-text-muted hover:text-apex-text-secondary'
            )}
            title="Timeline view"
          >
            <GanttChart size={16} />
          </button>
        </div>

        {/* Count */}
        <span className="text-[10px] font-mono text-apex-text-muted ml-auto">
          {viewMode === 'missions'
            ? `${missions.length} ${missions.length === 1 ? 'mission' : 'missions'}`
            : `${filteredTasks.length} ${filteredTasks.length === 1 ? 'task' : 'tasks'}`}
        </span>
      </div>

      {/* ── Content ───────────────────────────────────────────────── */}

      {/* Timeline View */}
      {viewMode === 'timeline' && (
        <div className="rounded-xl border border-apex-border-subtle/40 bg-apex-bg-secondary/30 backdrop-blur-sm p-4">
          <TaskTimeline
            tasks={filteredTasks}
            onTaskClick={(taskId) =>
              setExpandedMission(expandedMission === taskId ? null : taskId)
            }
          />
        </div>
      )}

      {/* Missions View */}
      {viewMode === 'missions' && (
        <div className="space-y-2">
          {missions.length > 0 ? (
            missions.map((mission) => (
              <MissionCard
                key={mission.dagId}
                mission={mission}
                agents={agents}
                expanded={expandedMission === mission.dagId}
                onToggle={() =>
                  setExpandedMission(
                    expandedMission === mission.dagId ? null : mission.dagId
                  )
                }
                onCancelTask={handleCancel}
                onRetryTask={handleRetry}
              />
            ))
          ) : (
            <SwarmFlowDiagram onLaunch={() => setShowCreate(true)} />
          )}
        </div>
      )}

      {/* Tasks View (flat list) */}
      {viewMode === 'tasks' && (
        <div className="space-y-1.5">
          <AnimatePresence>
            {filteredTasks.map((task, i) => {
              const cfg = STATUS_ICON[task.status]
              const StatusIcon = cfg.icon

              return (
                <motion.div
                  key={task.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ delay: i * 0.02 }}
                  className="rounded-lg border border-apex-border-subtle/30 bg-apex-bg-secondary/30 backdrop-blur-sm"
                >
                  <div className="flex items-center gap-4 px-4 py-3">
                    <div className={cn('p-1.5 rounded-md', cfg.bg)}>
                      <StatusIcon size={14} className={cfg.color} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">
                        {task.name}
                      </div>
                      <div className="flex items-center gap-2 text-[10px] text-apex-text-muted font-mono">
                        <span>{task.id.slice(0, 8)}</span>
                        <span className="text-apex-border-subtle">|</span>
                        <span>{formatDate(task.createdAt)}</span>
                      </div>
                    </div>
                    <span
                      className={cn(
                        'px-2 py-0.5 rounded text-[10px] font-mono uppercase tracking-wider',
                        cfg.bg,
                        cfg.color
                      )}
                    >
                      {cfg.label}
                    </span>
                    <div className="hidden md:flex items-center gap-5 text-xs font-mono text-apex-text-tertiary">
                      <span>{formatTokens(task.tokensUsed)} tok</span>
                      <span>{formatCost(task.costDollars)}</span>
                      {task.startedAt && task.completedAt && (
                        <span className="text-apex-text-muted">
                          {formatDuration(
                            new Date(task.completedAt).getTime() -
                              new Date(task.startedAt).getTime()
                          )}
                        </span>
                      )}
                    </div>
                    {task.status === 'running' && (
                      <button
                        onClick={() => handleCancel(task.id)}
                        className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono bg-red-500/10 text-red-400 hover:bg-red-500/20 border border-red-500/20 rounded transition-colors"
                      >
                        <XCircle size={10} />
                        Abort
                      </button>
                    )}
                    {task.status === 'failed' && (
                      <button
                        onClick={() => handleRetry(task.id)}
                        className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 border border-blue-500/20 rounded transition-colors"
                      >
                        <RotateCcw size={10} />
                        Retry
                      </button>
                    )}
                  </div>
                </motion.div>
              )
            })}
          </AnimatePresence>

          {filteredTasks.length === 0 && (
            <SwarmFlowDiagram onLaunch={() => setShowCreate(true)} />
          )}
        </div>
      )}

      {/* ── Launch Mission Modal ─────────────────────────────────── */}
      <AnimatePresence>
        {showCreate && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
            onClick={() => setShowCreate(false)}
          >
            <motion.div
              initial={{ scale: 0.95, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.95, opacity: 0 }}
              onClick={(e) => e.stopPropagation()}
              className="w-full max-w-lg p-6 bg-apex-bg-secondary border border-apex-border-subtle/50 rounded-xl shadow-2xl space-y-5"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="p-1.5 rounded-md bg-purple-500/10 border border-purple-500/20">
                    <Zap size={16} className="text-purple-400" />
                  </div>
                  <div>
                    <h2 className="text-base font-display font-semibold">
                      Launch Mission
                    </h2>
                    <p className="text-[10px] font-mono text-apex-text-muted uppercase tracking-wider">
                      Mission Briefing
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => setShowCreate(false)}
                  className="p-1 hover:bg-apex-bg-tertiary rounded-md transition-colors"
                >
                  <X size={16} className="text-apex-text-muted" />
                </button>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">
                    Mission Name
                  </label>
                  <input
                    type="text"
                    value={newTask.name}
                    onChange={(e) =>
                      setNewTask({ ...newTask, name: e.target.value })
                    }
                    placeholder="e.g. Analyze quarterly data across sources"
                    className="w-full px-3 py-2 text-sm bg-apex-bg-primary border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-purple-500/50 font-mono transition-colors"
                    autoFocus
                  />
                </div>

                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">
                    Objective
                  </label>
                  <textarea
                    value={newTask.prompt}
                    onChange={(e) =>
                      setNewTask({ ...newTask, prompt: e.target.value })
                    }
                    placeholder="Describe the mission objective. The swarm will decompose and parallelize execution across available agents..."
                    rows={4}
                    className="w-full px-3 py-2 text-sm bg-apex-bg-primary border border-apex-border-subtle/50 rounded-lg focus:outline-none focus:border-purple-500/50 font-mono resize-none transition-colors"
                  />
                </div>

                <div>
                  <label className="block text-[10px] font-mono text-apex-text-muted uppercase tracking-wider mb-1.5">
                    Priority Level{' '}
                    <span className="text-apex-text-muted/50">(1-10)</span>
                  </label>
                  <div className="flex items-center gap-3">
                    <input
                      type="range"
                      min={1}
                      max={10}
                      value={newTask.priority}
                      onChange={(e) =>
                        setNewTask({
                          ...newTask,
                          priority: parseInt(e.target.value) || 5,
                        })
                      }
                      className="flex-1 accent-purple-500"
                    />
                    <span
                      className={cn(
                        'stat-value text-lg font-semibold w-8 text-center',
                        newTask.priority >= 8
                          ? 'text-red-400'
                          : newTask.priority >= 5
                          ? 'text-amber-400'
                          : 'text-blue-400'
                      )}
                    >
                      {newTask.priority}
                    </span>
                  </div>
                  <p className="text-[10px] font-mono text-apex-text-muted mt-1">
                    The swarm will decompose and execute through parallel agents
                  </p>
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-1">
                <button
                  onClick={() => setShowCreate(false)}
                  className="px-3 py-1.5 text-xs font-mono text-apex-text-muted hover:text-apex-text-secondary transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreate}
                  disabled={
                    creating ||
                    !newTask.name.trim() ||
                    !newTask.prompt.trim()
                  }
                  className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-purple-500/10 hover:bg-purple-500/20 text-purple-400 border border-purple-500/20 rounded-lg transition-all disabled:opacity-40"
                >
                  {creating && (
                    <Loader2 size={14} className="animate-spin" />
                  )}
                  Launch
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
