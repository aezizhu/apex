import { ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { motion } from 'framer-motion'
import {
  LayoutDashboard,
  Users,
  ListTodo,
  ShieldCheck,
  Settings,
  Menu,
  X,
  Activity,
  Eye,
  Wifi,
  WifiOff,
} from 'lucide-react'
import { useStore } from '../lib/store'
import { cn } from '../lib/utils'

interface LayoutProps {
  children: ReactNode
}

const navItems = [
  { path: '/', icon: LayoutDashboard, label: 'Dashboard' },
  { path: '/agents', icon: Users, label: 'Agents' },
  { path: '/agent-sight', icon: Eye, label: 'Agent Sight' },
  { path: '/tasks', icon: ListTodo, label: 'Tasks' },
  { path: '/approvals', icon: ShieldCheck, label: 'Approvals' },
  { path: '/settings', icon: Settings, label: 'Settings' },
]

export default function Layout({ children }: LayoutProps) {
  const { wsConnected, sidebarCollapsed, setSidebarCollapsed, metrics } = useStore()

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Sidebar */}
      <motion.aside
        initial={false}
        animate={{ width: sidebarCollapsed ? 64 : 240 }}
        className="flex flex-col bg-apex-bg-secondary border-r border-apex-border-subtle"
      >
        {/* Logo */}
        <div className="flex items-center justify-between h-16 px-4 border-b border-apex-border-subtle">
          {!sidebarCollapsed && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="flex items-center gap-2"
            >
              <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center">
                <Activity className="w-5 h-5 text-white" />
              </div>
              <span className="font-semibold text-lg">Apex</span>
            </motion.div>
          )}
          <button
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            className="p-2 hover:bg-apex-bg-tertiary rounded-lg transition-colors"
          >
            {sidebarCollapsed ? <Menu size={20} /> : <X size={20} />}
          </button>
        </div>

        {/* Navigation */}
        <nav className="flex-1 py-4 px-2 space-y-1">
          {navItems.map(({ path, icon: Icon, label }) => (
            <NavLink
              key={path}
              to={path}
              className={({ isActive }) =>
                cn(
                  'flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all',
                  'hover:bg-apex-bg-tertiary',
                  isActive
                    ? 'bg-apex-accent-primary/10 text-apex-accent-primary'
                    : 'text-apex-text-secondary'
                )
              }
            >
              <Icon size={20} />
              {!sidebarCollapsed && (
                <motion.span
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  className="font-medium"
                >
                  {label}
                </motion.span>
              )}
            </NavLink>
          ))}
        </nav>

        {/* Connection Status */}
        <div className="p-4 border-t border-apex-border-subtle">
          <div className="flex items-center gap-2 text-sm">
            {wsConnected ? (
              <>
                <Wifi size={16} className="text-green-500" />
                {!sidebarCollapsed && (
                  <span className="text-apex-text-secondary">Connected</span>
                )}
              </>
            ) : (
              <>
                <WifiOff size={16} className="text-red-500" />
                {!sidebarCollapsed && (
                  <span className="text-apex-text-secondary">Disconnected</span>
                )}
              </>
            )}
          </div>
        </div>
      </motion.aside>

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top Bar */}
        <header className="h-16 flex items-center justify-between px-6 border-b border-apex-border-subtle bg-apex-bg-secondary">
          <div className="flex items-center gap-6">
            {/* Quick Stats */}
            <div className="flex items-center gap-4 text-sm">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-green-500" />
                <span className="text-apex-text-secondary">
                  {metrics.activeAgents} active agents
                </span>
              </div>
              <div className="text-apex-border-default">|</div>
              <div className="text-apex-text-secondary">
                {metrics.runningTasks} running tasks
              </div>
              <div className="text-apex-border-default">|</div>
              <div className="text-apex-text-secondary">
                ${metrics.totalCost.toFixed(4)} spent
              </div>
            </div>
          </div>

          <div className="flex items-center gap-4">
            {/* Success Rate Indicator */}
            <div className="flex items-center gap-2">
              <span className="text-sm text-apex-text-secondary">Success Rate:</span>
              <span
                className={cn(
                  'text-sm font-medium',
                  metrics.successRate >= 0.95
                    ? 'text-green-500'
                    : metrics.successRate >= 0.8
                    ? 'text-yellow-500'
                    : 'text-red-500'
                )}
              >
                {(metrics.successRate * 100).toFixed(1)}%
              </span>
            </div>
          </div>
        </header>

        {/* Page Content */}
        <main className="flex-1 overflow-auto bg-apex-bg-primary p-6">
          {children}
        </main>
      </div>
    </div>
  )
}
