import { useState, useEffect, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  ShieldCheck,
  ShieldAlert,
  Check,
  X,
  Clock,
  AlertTriangle,
  ChevronDown,
} from 'lucide-react'
import { useStore, selectPendingApprovals } from '../lib/store'
import { cn, formatDate } from '../lib/utils'
import { approvalApi } from '../lib/api'
import toast from 'react-hot-toast'

export default function Approvals() {
  const approvals = useStore((s) => s.approvals)
  const pendingApprovals = useStore(selectPendingApprovals)
  const updateApproval = useStore((s) => s.updateApproval)
  const setApprovals = useStore((s) => s.setApprovals)
  const [expandedApproval, setExpandedApproval] = useState<string | null>(null)
  const [filter, setFilter] = useState<'all' | 'pending' | 'resolved'>('pending')

  const fetchApprovals = useCallback(async () => {
    try {
      const response = await approvalApi.list({ pageSize: 100 })
      if (Array.isArray(response.data)) {
        setApprovals(response.data)
      }
    } catch {
      // WebSocket will keep data flowing
    }
  }, [setApprovals])

  useEffect(() => {
    fetchApprovals()
  }, [fetchApprovals])

  const filteredApprovals = approvals.filter((a) => {
    if (filter === 'pending') return a.status === 'pending'
    if (filter === 'resolved') return a.status !== 'pending'
    return true
  })

  const handleApprove = async (id: string) => {
    try {
      await approvalApi.approve(id)
      updateApproval(id, 'approved')
      toast.success('Request approved')
    } catch {
      toast.error('Failed to approve request')
    }
  }

  const handleDeny = async (id: string) => {
    try {
      await approvalApi.deny(id)
      updateApproval(id, 'denied')
      toast.success('Request denied')
    } catch {
      toast.error('Failed to deny request')
    }
  }

  const handleBulkApprove = async () => {
    try {
      await Promise.all(pendingApprovals.map((a) => approvalApi.approve(a.id)))
      pendingApprovals.forEach((a) => updateApproval(a.id, 'approved'))
      toast.success(`Approved ${pendingApprovals.length} requests`)
    } catch {
      toast.error('Failed to approve some requests')
    }
  }

  const getRiskColor = (score: number) => {
    if (score >= 0.8) return 'text-red-500 bg-red-500/10'
    if (score >= 0.5) return 'text-yellow-500 bg-yellow-500/10'
    return 'text-green-500 bg-green-500/10'
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Approval Queue</h1>
          <p className="text-apex-text-secondary">
            Review and approve high-impact agent actions
          </p>
        </div>
        {pendingApprovals.length > 0 && (
          <div className="flex items-center gap-2">
            <button
              onClick={handleBulkApprove}
              className="flex items-center gap-2 px-4 py-2 bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded-lg transition-colors"
            >
              <Check size={18} />
              Approve All ({pendingApprovals.length})
            </button>
          </div>
        )}
      </div>

      {/* Stats */}
      <div className="grid grid-cols-3 gap-4">
        <div className="bg-apex-bg-secondary rounded-lg border border-apex-border-subtle p-4">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-yellow-500/10">
              <Clock size={20} className="text-yellow-500" />
            </div>
            <div>
              <div className="text-2xl font-bold">{pendingApprovals.length}</div>
              <div className="text-sm text-apex-text-secondary">Pending</div>
            </div>
          </div>
        </div>
        <div className="bg-apex-bg-secondary rounded-lg border border-apex-border-subtle p-4">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-green-500/10">
              <ShieldCheck size={20} className="text-green-500" />
            </div>
            <div>
              <div className="text-2xl font-bold">
                {approvals.filter((a) => a.status === 'approved').length}
              </div>
              <div className="text-sm text-apex-text-secondary">Approved</div>
            </div>
          </div>
        </div>
        <div className="bg-apex-bg-secondary rounded-lg border border-apex-border-subtle p-4">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-red-500/10">
              <ShieldAlert size={20} className="text-red-500" />
            </div>
            <div>
              <div className="text-2xl font-bold">
                {approvals.filter((a) => a.status === 'denied').length}
              </div>
              <div className="text-sm text-apex-text-secondary">Denied</div>
            </div>
          </div>
        </div>
      </div>

      {/* Filter */}
      <div className="flex items-center gap-2">
        {(['pending', 'resolved', 'all'] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={cn(
              'px-4 py-2 rounded-lg text-sm font-medium transition-colors',
              filter === f
                ? 'bg-apex-accent-primary text-white'
                : 'bg-apex-bg-secondary text-apex-text-secondary hover:bg-apex-bg-tertiary'
            )}
          >
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {/* Keyboard Shortcuts Hint */}
      <div className="text-sm text-apex-text-tertiary">
        Keyboard shortcuts: <kbd className="px-1.5 py-0.5 bg-apex-bg-tertiary rounded">j</kbd>/
        <kbd className="px-1.5 py-0.5 bg-apex-bg-tertiary rounded">k</kbd> navigate,{' '}
        <kbd className="px-1.5 py-0.5 bg-apex-bg-tertiary rounded">a</kbd> approve,{' '}
        <kbd className="px-1.5 py-0.5 bg-apex-bg-tertiary rounded">d</kbd> deny
      </div>

      {/* Approval List */}
      <div className="space-y-2">
        <AnimatePresence>
          {filteredApprovals.map((approval, i) => (
            <motion.div
              key={approval.id}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ delay: i * 0.02 }}
              className={cn(
                'bg-apex-bg-secondary rounded-lg border overflow-hidden',
                approval.status === 'pending'
                  ? 'border-yellow-500/30'
                  : 'border-apex-border-subtle'
              )}
            >
              {/* Approval Header */}
              <div
                className="flex items-center gap-4 px-4 py-3 cursor-pointer hover:bg-apex-bg-tertiary/50 transition-colors"
                onClick={() =>
                  setExpandedApproval(
                    expandedApproval === approval.id ? null : approval.id
                  )
                }
              >
                {/* Risk Indicator */}
                <div
                  className={cn(
                    'p-2 rounded-lg',
                    getRiskColor(approval.riskScore)
                  )}
                >
                  <AlertTriangle size={18} />
                </div>

                <div className="flex-1 min-w-0">
                  <div className="font-medium">{approval.actionType}</div>
                  <div className="text-xs text-apex-text-tertiary font-mono">
                    Agent: {approval.agentId.slice(0, 8)} • Task: {approval.taskId.slice(0, 8)}
                  </div>
                </div>

                <div className="flex items-center gap-4">
                  {/* Risk Score */}
                  <div
                    className={cn(
                      'px-2 py-1 rounded text-xs font-medium',
                      getRiskColor(approval.riskScore)
                    )}
                  >
                    Risk: {(approval.riskScore * 100).toFixed(0)}%
                  </div>

                  {/* Status or Actions */}
                  {approval.status === 'pending' ? (
                    <div className="flex items-center gap-2">
                      <button
                        onClick={(e) => {
                          e.stopPropagation()
                          handleApprove(approval.id)
                        }}
                        className="p-2 bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded-lg transition-colors"
                      >
                        <Check size={18} />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation()
                          handleDeny(approval.id)
                        }}
                        className="p-2 bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded-lg transition-colors"
                      >
                        <X size={18} />
                      </button>
                    </div>
                  ) : (
                    <span
                      className={cn(
                        'px-2 py-1 rounded text-xs font-medium',
                        approval.status === 'approved' &&
                          'bg-green-500/10 text-green-500',
                        approval.status === 'denied' && 'bg-red-500/10 text-red-500'
                      )}
                    >
                      {approval.status}
                    </span>
                  )}

                  <ChevronDown
                    size={18}
                    className={cn(
                      'text-apex-text-tertiary transition-transform',
                      expandedApproval === approval.id && 'rotate-180'
                    )}
                  />
                </div>
              </div>

              {/* Expanded Details */}
              <AnimatePresence>
                {expandedApproval === approval.id && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: 'auto', opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    className="border-t border-apex-border-subtle"
                  >
                    <div className="p-4 space-y-4">
                      <div>
                        <div className="text-xs text-apex-text-tertiary mb-2">
                          Action Data
                        </div>
                        <pre className="bg-apex-bg-primary rounded-lg p-3 text-sm overflow-x-auto">
                          {JSON.stringify(approval.actionData, null, 2)}
                        </pre>
                      </div>
                      <div className="text-xs text-apex-text-tertiary">
                        Created: {formatDate(approval.createdAt)}
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </motion.div>
          ))}
        </AnimatePresence>

        {filteredApprovals.length === 0 && (
          <div className="text-center py-12 text-apex-text-secondary">
            {filter === 'pending'
              ? 'No pending approvals'
              : 'No approvals found'}
          </div>
        )}
      </div>
    </div>
  )
}
