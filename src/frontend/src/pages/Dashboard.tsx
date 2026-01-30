import { useEffect, useCallback } from 'react'
import { motion } from 'framer-motion'
import {
  Activity,
  Users,
  DollarSign,
  Zap,
  TrendingUp,
  Clock,
  CheckCircle,
} from 'lucide-react'
import { useStore, selectAgentList, selectTaskList } from '../lib/store'
import { cn, formatCost, formatTokens, formatDuration } from '../lib/utils'
import { agentApi, taskApi, metricsApi } from '../lib/api'
import AgentGrid from '../components/agents/AgentGrid'
import MetricsChart from '../components/metrics/MetricsChart'

const statCards = [
  {
    title: 'Active Agents',
    key: 'activeAgents' as const,
    icon: Users,
    color: 'blue',
  },
  {
    title: 'Running Tasks',
    key: 'runningTasks' as const,
    icon: Activity,
    color: 'green',
  },
  {
    title: 'Total Cost',
    key: 'totalCost' as const,
    icon: DollarSign,
    color: 'purple',
    format: formatCost,
  },
  {
    title: 'Total Tokens',
    key: 'totalTokens' as const,
    icon: Zap,
    color: 'amber',
    format: formatTokens,
  },
]

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

  // Refresh on mount
  useEffect(() => {
    refreshData()
  }, [refreshData])

  const recentTasks = tasks
    .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
    .slice(0, 5)

  const completedToday = tasks.filter(
    (t) =>
      t.status === 'completed' &&
      new Date(t.completedAt || '').toDateString() === new Date().toDateString()
  ).length

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Dashboard</h1>
          <p className="text-apex-text-secondary">
            Real-time overview of your agent swarm
          </p>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          <span className="text-apex-text-secondary">Live</span>
        </div>
      </div>

      {/* Stat Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {statCards.map(({ title, key, icon: Icon, color, format }, i) => (
          <motion.div
            key={key}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: i * 0.1 }}
            className={cn(
              'p-5 rounded-xl border border-apex-border-subtle',
              'bg-gradient-to-br from-apex-bg-secondary to-apex-bg-tertiary'
            )}
          >
            <div className="flex items-center justify-between mb-3">
              <div
                className={cn(
                  'p-2 rounded-lg',
                  color === 'blue' && 'bg-blue-500/10',
                  color === 'green' && 'bg-green-500/10',
                  color === 'purple' && 'bg-purple-500/10',
                  color === 'amber' && 'bg-amber-500/10'
                )}
              >
                <Icon
                  size={20}
                  className={cn(
                    color === 'blue' && 'text-blue-500',
                    color === 'green' && 'text-green-500',
                    color === 'purple' && 'text-purple-500',
                    color === 'amber' && 'text-amber-500'
                  )}
                />
              </div>
              <TrendingUp size={16} className="text-green-500" />
            </div>
            <div className="text-2xl font-bold mb-1">
              {format ? format(metrics[key] as number) : metrics[key]}
            </div>
            <div className="text-sm text-apex-text-secondary">{title}</div>
          </motion.div>
        ))}
      </div>

      {/* Main Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Agent Grid */}
        <div className="lg:col-span-2 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <div className="flex items-center justify-between mb-4">
            <h2 className="font-semibold">Agent Swarm</h2>
            <span className="text-sm text-apex-text-secondary">
              {agents.filter((a) => a.status === 'busy').length} busy
            </span>
          </div>
          <div className="h-[400px]">
            <AgentGrid maxAgents={200} />
          </div>
        </div>

        {/* Right Sidebar */}
        <div className="space-y-6">
          {/* Quick Stats */}
          <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
            <h2 className="font-semibold mb-4">Today's Summary</h2>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <CheckCircle size={18} className="text-green-500" />
                  <span className="text-apex-text-secondary">Completed</span>
                </div>
                <span className="font-semibold">{completedToday}</span>
              </div>
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Clock size={18} className="text-blue-500" />
                  <span className="text-apex-text-secondary">Avg. Latency</span>
                </div>
                <span className="font-semibold">
                  {formatDuration(metrics.avgLatencyMs)}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Activity size={18} className="text-purple-500" />
                  <span className="text-apex-text-secondary">Success Rate</span>
                </div>
                <span
                  className={cn(
                    'font-semibold',
                    metrics.successRate >= 0.95 && 'text-green-500',
                    metrics.successRate >= 0.8 &&
                      metrics.successRate < 0.95 &&
                      'text-yellow-500',
                    metrics.successRate < 0.8 && 'text-red-500'
                  )}
                >
                  {(metrics.successRate * 100).toFixed(1)}%
                </span>
              </div>
            </div>
          </div>

          {/* Recent Tasks */}
          <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
            <h2 className="font-semibold mb-4">Recent Tasks</h2>
            <div className="space-y-3">
              {recentTasks.length === 0 ? (
                <p className="text-apex-text-secondary text-sm">No recent tasks</p>
              ) : (
                recentTasks.map((task) => (
                  <div
                    key={task.id}
                    className="flex items-center justify-between py-2 border-b border-apex-border-subtle last:border-0"
                  >
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{task.name}</div>
                      <div className="text-xs text-apex-text-tertiary">
                        {formatCost(task.costDollars)} • {formatTokens(task.tokensUsed)} tokens
                      </div>
                    </div>
                    <div
                      className={cn(
                        'px-2 py-1 rounded text-xs font-medium',
                        task.status === 'completed' && 'bg-green-500/10 text-green-500',
                        task.status === 'running' && 'bg-blue-500/10 text-blue-500',
                        task.status === 'failed' && 'bg-red-500/10 text-red-500',
                        task.status === 'pending' && 'bg-yellow-500/10 text-yellow-500'
                      )}
                    >
                      {task.status}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Metrics Chart */}
      <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
        <h2 className="font-semibold mb-4">Performance Trends</h2>
        <div className="h-[300px]">
          <MetricsChart />
        </div>
      </div>
    </div>
  )
}
