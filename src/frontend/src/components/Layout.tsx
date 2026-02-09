import { ReactNode } from 'react'
import { NavLink, useLocation } from 'react-router-dom'
import { motion, AnimatePresence } from 'framer-motion'
import {
  LayoutDashboard,
  Users,
  ListTodo,
  ShieldCheck,
  Settings,
  Eye,
  Wifi,
  WifiOff,
  GitBranch,
  Hexagon,
} from 'lucide-react'
import { useStore } from '../lib/store'
import { cn, formatCost } from '../lib/utils'

interface LayoutProps {
  children: ReactNode
}

const navItems = [
  { path: '/', icon: LayoutDashboard, label: 'Command', tag: null },
  { path: '/agents', icon: Users, label: 'Agents', tag: 'agents' },
  { path: '/agent-sight', icon: Eye, label: 'Sight', tag: null },
  { path: '/tasks', icon: ListTodo, label: 'Operations', tag: 'tasks' },
  { path: '/approvals', icon: ShieldCheck, label: 'Approvals', tag: 'approvals' },
  { path: '/workflows', icon: GitBranch, label: 'Workflows', tag: null },
  { path: '/settings', icon: Settings, label: 'Settings', tag: null },
]

export default function Layout({ children }: LayoutProps) {
  const { wsConnected, sidebarCollapsed, setSidebarCollapsed, metrics, approvals } = useStore()
  const location = useLocation()

  const pendingApprovals = approvals.filter((a) => a.status === 'pending').length

  const getTag = (tag: string | null): number | null => {
    if (!tag) return null
    if (tag === 'agents') return metrics.activeAgents || null
    if (tag === 'tasks') return metrics.runningTasks || null
    if (tag === 'approvals') return pendingApprovals || null
    return null
  }

  return (
    <div className="flex h-screen overflow-hidden bg-apex-bg-primary">
      {/* Sidebar */}
      <motion.aside
        initial={false}
        animate={{ width: sidebarCollapsed ? 60 : 220 }}
        transition={{ duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
        className="flex flex-col border-r border-apex-border-subtle/50 relative z-20"
        style={{
          background: 'linear-gradient(180deg, #0e0e16 0%, #0a0a0f 100%)',
        }}
      >
        {/* Logo */}
        <div className="flex items-center h-14 px-3 glow-bar">
          <button
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            className="flex items-center gap-2.5 p-1.5 rounded-lg hover:bg-white/[0.04] transition-colors w-full"
          >
            <div className="w-8 h-8 rounded-lg flex items-center justify-center relative shrink-0">
              <div className="absolute inset-0 rounded-lg bg-gradient-to-br from-blue-500/20 to-purple-600/20" />
              <Hexagon size={18} className="text-blue-400 relative z-10" strokeWidth={2.5} />
            </div>
            <AnimatePresence mode="wait">
              {!sidebarCollapsed && (
                <motion.div
                  initial={{ opacity: 0, x: -8 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -8 }}
                  transition={{ duration: 0.15 }}
                  className="flex items-center gap-1.5 min-w-0"
                >
                  <span className="font-display font-bold text-[15px] tracking-tight">APEX</span>
                  <span className="text-[10px] font-mono text-apex-text-tertiary bg-white/[0.04] px-1.5 py-0.5 rounded">
                    v0.1
                  </span>
                </motion.div>
              )}
            </AnimatePresence>
          </button>
        </div>

        {/* Navigation */}
        <nav className="flex-1 py-3 px-2 space-y-0.5">
          {navItems.map(({ path, icon: Icon, label, tag }) => {
            const isActive = path === '/' ? location.pathname === '/' : location.pathname.startsWith(path)
            const count = getTag(tag)

            return (
              <NavLink
                key={path}
                to={path}
                className={cn(
                  'flex items-center gap-2.5 px-2.5 py-2 rounded-lg transition-all group relative',
                  isActive
                    ? 'bg-white/[0.06] text-white'
                    : 'text-apex-text-tertiary hover:text-apex-text-secondary hover:bg-white/[0.02]'
                )}
              >
                {isActive && (
                  <motion.div
                    layoutId="nav-indicator"
                    className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-5 bg-blue-400 rounded-r-full"
                    transition={{ type: 'spring', stiffness: 500, damping: 35 }}
                  />
                )}
                <Icon size={18} strokeWidth={isActive ? 2 : 1.5} className="shrink-0" />
                <AnimatePresence mode="wait">
                  {!sidebarCollapsed && (
                    <motion.div
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.1 }}
                      className="flex items-center justify-between flex-1 min-w-0"
                    >
                      <span className={cn('text-[13px]', isActive ? 'font-semibold' : 'font-medium')}>
                        {label}
                      </span>
                      {count !== null && count > 0 && (
                        <span className="text-[10px] font-mono bg-blue-500/15 text-blue-400 px-1.5 py-0.5 rounded-full">
                          {count}
                        </span>
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>
              </NavLink>
            )
          })}
        </nav>

        {/* Connection Status */}
        <div className="p-3 border-t border-apex-border-subtle/30">
          <div className="flex items-center gap-2">
            {wsConnected ? (
              <div className="flex items-center gap-2">
                <div className="relative">
                  <Wifi size={14} className="text-emerald-400" />
                  <div className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 bg-emerald-400 rounded-full animate-pulse" />
                </div>
                <AnimatePresence>
                  {!sidebarCollapsed && (
                    <motion.span
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      className="text-[11px] text-emerald-400/80 font-mono"
                    >
                      LIVE
                    </motion.span>
                  )}
                </AnimatePresence>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <WifiOff size={14} className="text-red-400/60" />
                <AnimatePresence>
                  {!sidebarCollapsed && (
                    <motion.span
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      className="text-[11px] text-red-400/60 font-mono"
                    >
                      OFFLINE
                    </motion.span>
                  )}
                </AnimatePresence>
              </div>
            )}
          </div>
        </div>
      </motion.aside>

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top Bar — minimal telemetry strip */}
        <header className="h-11 flex items-center justify-between px-5 border-b border-apex-border-subtle/30 bg-apex-bg-primary/80 backdrop-blur-sm relative z-10">
          <div className="flex items-center gap-5">
            <div className="flex items-center gap-1.5 text-[11px] font-mono">
              <div className={cn(
                'w-1.5 h-1.5 rounded-full',
                metrics.activeAgents > 0 ? 'bg-emerald-400 animate-pulse' : 'bg-apex-text-muted'
              )} />
              <span className="text-apex-text-tertiary">AGENTS</span>
              <span className="text-apex-text-secondary font-semibold">{metrics.activeAgents}/{metrics.totalAgents}</span>
            </div>

            <div className="w-px h-3 bg-apex-border-subtle/50" />

            <div className="flex items-center gap-1.5 text-[11px] font-mono">
              <div className={cn(
                'w-1.5 h-1.5 rounded-full',
                metrics.runningTasks > 0 ? 'bg-blue-400 animate-pulse' : 'bg-apex-text-muted'
              )} />
              <span className="text-apex-text-tertiary">TASKS</span>
              <span className="text-apex-text-secondary font-semibold">{metrics.runningTasks}</span>
            </div>

            <div className="w-px h-3 bg-apex-border-subtle/50" />

            <div className="flex items-center gap-1.5 text-[11px] font-mono">
              <span className="text-apex-text-tertiary">COST</span>
              <span className="text-apex-text-secondary font-semibold">{formatCost(metrics.totalCost)}</span>
            </div>
          </div>

          <div className="flex items-center gap-4">
            <div className="flex items-center gap-1.5 text-[11px] font-mono">
              <span className="text-apex-text-tertiary">SUCCESS</span>
              <span
                className={cn(
                  'font-semibold',
                  metrics.successRate >= 0.95
                    ? 'text-emerald-400'
                    : metrics.successRate >= 0.8
                    ? 'text-amber-400'
                    : 'text-red-400'
                )}
              >
                {(metrics.successRate * 100).toFixed(1)}%
              </span>
            </div>
          </div>
        </header>

        {/* Page Content */}
        <main className="flex-1 overflow-auto bg-apex-bg-primary">
          {children}
        </main>
      </div>
    </div>
  )
}
