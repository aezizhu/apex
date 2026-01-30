import { useState, useEffect, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Plus,
  Search,
  Clock,
  CheckCircle,
  XCircle,
  PlayCircle,
  PauseCircle,
  ChevronDown,
  List,
  GanttChart,
} from 'lucide-react'
import { useStore, selectTaskList, type Task } from '../lib/store'
import { cn, formatCost, formatTokens, formatDate, formatDuration } from '../lib/utils'
import { taskApi } from '../lib/api'
import { TaskTimeline } from '../components/TaskTimeline'
import toast from 'react-hot-toast'

type ViewMode = 'list' | 'timeline'

export default function Tasks() {
  const tasks = useStore(selectTaskList)
  const setTasks = useStore((s) => s.setTasks)
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<Task['status'] | 'all'>('all')
  const [expandedTask, setExpandedTask] = useState<string | null>(null)
  const [viewMode, setViewMode] = useState<ViewMode>('list')

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

  const handleCancel = useCallback(async (taskId: string) => {
    try {
      await taskApi.cancel(taskId)
      toast.success('Task cancelled')
      fetchTasks()
    } catch {
      toast.error('Failed to cancel task')
    }
  }, [fetchTasks])

  const handleRetry = useCallback(async (taskId: string) => {
    try {
      await taskApi.retry(taskId)
      toast.success('Task retrying')
      fetchTasks()
    } catch {
      toast.error('Failed to retry task')
    }
  }, [fetchTasks])

  const filteredTasks = tasks
    .filter((task) => {
      const matchesSearch = task.name.toLowerCase().includes(searchQuery.toLowerCase())
      const matchesStatus = statusFilter === 'all' || task.status === statusFilter
      return matchesSearch && matchesStatus
    })
    .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())

  const getStatusIcon = (status: Task['status']) => {
    switch (status) {
      case 'completed':
        return <CheckCircle size={18} className="text-green-500" />
      case 'failed':
        return <XCircle size={18} className="text-red-500" />
      case 'running':
        return <PlayCircle size={18} className="text-blue-500" />
      case 'pending':
      case 'ready':
        return <Clock size={18} className="text-yellow-500" />
      case 'cancelled':
        return <PauseCircle size={18} className="text-gray-500" />
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Tasks</h1>
          <p className="text-apex-text-secondary">
            Monitor and manage task execution
          </p>
        </div>
        <button className="flex items-center gap-2 px-4 py-2 bg-apex-accent-primary hover:bg-blue-600 text-white rounded-lg transition-colors">
          <Plus size={18} />
          Submit Task
        </button>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {[
          { label: 'Pending', value: tasks.filter((t) => t.status === 'pending').length, color: 'yellow' },
          { label: 'Running', value: tasks.filter((t) => t.status === 'running').length, color: 'blue' },
          { label: 'Completed', value: tasks.filter((t) => t.status === 'completed').length, color: 'green' },
          { label: 'Failed', value: tasks.filter((t) => t.status === 'failed').length, color: 'red' },
        ].map(({ label, value, color }) => (
          <div
            key={label}
            className="bg-apex-bg-secondary rounded-lg border border-apex-border-subtle p-4"
          >
            <div className="text-2xl font-bold">{value}</div>
            <div
              className={cn(
                'text-sm',
                color === 'yellow' && 'text-yellow-500',
                color === 'blue' && 'text-blue-500',
                color === 'green' && 'text-green-500',
                color === 'red' && 'text-red-500'
              )}
            >
              {label}
            </div>
          </div>
        ))}
      </div>

      {/* Filters + View Toggle */}
      <div className="flex flex-wrap items-center gap-4">
        <div className="relative flex-1 min-w-[200px] max-w-md">
          <Search
            size={18}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-apex-text-tertiary"
          />
          <input
            type="text"
            placeholder="Search tasks..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
          />
        </div>

        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as Task['status'] | 'all')}
          className="px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
        >
          <option value="all">All Status</option>
          <option value="pending">Pending</option>
          <option value="running">Running</option>
          <option value="completed">Completed</option>
          <option value="failed">Failed</option>
          <option value="cancelled">Cancelled</option>
        </select>

        {/* View Toggle */}
        <div className="flex items-center gap-1 bg-apex-bg-secondary rounded-lg p-1">
          <button
            onClick={() => setViewMode('list')}
            className={cn(
              'p-2 rounded-md transition-colors',
              viewMode === 'list'
                ? 'bg-apex-bg-tertiary text-apex-text-primary'
                : 'text-apex-text-tertiary hover:text-apex-text-secondary'
            )}
            title="List view"
          >
            <List size={18} />
          </button>
          <button
            onClick={() => setViewMode('timeline')}
            className={cn(
              'p-2 rounded-md transition-colors',
              viewMode === 'timeline'
                ? 'bg-apex-bg-tertiary text-apex-text-primary'
                : 'text-apex-text-tertiary hover:text-apex-text-secondary'
            )}
            title="Timeline view"
          >
            <GanttChart size={18} />
          </button>
        </div>
      </div>

      {/* Content */}
      {viewMode === 'timeline' ? (
        <div className="bg-apex-bg-secondary rounded-xl border border-apex-border-subtle p-4">
          <TaskTimeline
            tasks={filteredTasks}
            onTaskClick={(taskId) =>
              setExpandedTask(expandedTask === taskId ? null : taskId)
            }
          />
        </div>
      ) : (
        /* Task List */
        <div className="space-y-2">
          <AnimatePresence>
            {filteredTasks.map((task, i) => (
              <motion.div
                key={task.id}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ delay: i * 0.02 }}
                className="bg-apex-bg-secondary rounded-lg border border-apex-border-subtle overflow-hidden"
              >
                {/* Task Header */}
                <div
                  className="flex items-center gap-4 px-4 py-3 cursor-pointer hover:bg-apex-bg-tertiary/50 transition-colors"
                  onClick={() =>
                    setExpandedTask(expandedTask === task.id ? null : task.id)
                  }
                >
                  {getStatusIcon(task.status)}

                  <div className="flex-1 min-w-0">
                    <div className="font-medium truncate">{task.name}</div>
                    <div className="text-xs text-apex-text-tertiary font-mono">
                      {task.id.slice(0, 8)} • {formatDate(task.createdAt)}
                    </div>
                  </div>

                  <div className="flex items-center gap-6 text-sm">
                    <div className="text-apex-text-secondary">
                      {formatTokens(task.tokensUsed)} tokens
                    </div>
                    <div className="text-apex-text-secondary font-mono">
                      {formatCost(task.costDollars)}
                    </div>
                    {task.startedAt && task.completedAt && (
                      <div className="text-apex-text-tertiary">
                        {formatDuration(
                          new Date(task.completedAt).getTime() -
                            new Date(task.startedAt).getTime()
                        )}
                      </div>
                    )}
                  </div>

                  <ChevronDown
                    size={18}
                    className={cn(
                      'text-apex-text-tertiary transition-transform',
                      expandedTask === task.id && 'rotate-180'
                    )}
                  />
                </div>

                {/* Expanded Details */}
                <AnimatePresence>
                  {expandedTask === task.id && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: 'auto', opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      className="border-t border-apex-border-subtle"
                    >
                      <div className="p-4 space-y-4">
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                          <div>
                            <div className="text-xs text-apex-text-tertiary mb-1">
                              DAG ID
                            </div>
                            <div className="text-sm font-mono">{task.dagId.slice(0, 12)}</div>
                          </div>
                          <div>
                            <div className="text-xs text-apex-text-tertiary mb-1">
                              Agent
                            </div>
                            <div className="text-sm font-mono">
                              {task.agentId?.slice(0, 12) || 'Unassigned'}
                            </div>
                          </div>
                          <div>
                            <div className="text-xs text-apex-text-tertiary mb-1">
                              Started
                            </div>
                            <div className="text-sm">
                              {task.startedAt ? formatDate(task.startedAt) : '-'}
                            </div>
                          </div>
                          <div>
                            <div className="text-xs text-apex-text-tertiary mb-1">
                              Completed
                            </div>
                            <div className="text-sm">
                              {task.completedAt ? formatDate(task.completedAt) : '-'}
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          <button className="px-3 py-1.5 text-sm bg-apex-bg-tertiary hover:bg-apex-bg-elevated rounded-lg transition-colors">
                            View Details
                          </button>
                          <button className="px-3 py-1.5 text-sm bg-apex-bg-tertiary hover:bg-apex-bg-elevated rounded-lg transition-colors">
                            View Logs
                          </button>
                          {task.status === 'running' && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                handleCancel(task.id)
                              }}
                              className="px-3 py-1.5 text-sm bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded-lg transition-colors"
                            >
                              Cancel
                            </button>
                          )}
                          {task.status === 'failed' && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                handleRetry(task.id)
                              }}
                              className="px-3 py-1.5 text-sm bg-blue-500/10 text-blue-500 hover:bg-blue-500/20 rounded-lg transition-colors"
                            >
                              Retry
                            </button>
                          )}
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            ))}
          </AnimatePresence>

          {filteredTasks.length === 0 && (
            <div className="text-center py-12 text-apex-text-secondary">
              No tasks found
            </div>
          )}
        </div>
      )}
    </div>
  )
}
