import { useEffect, useRef, useCallback, useState } from 'react'
import { useStore } from '../lib/store'
import toast from 'react-hot-toast'
import { parseWsMessage, type WsServerMessage } from '../types'

const WS_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws'
const RECONNECT_DELAY = 3000
const MAX_RECONNECT_ATTEMPTS = 10
const SESSION_STORAGE_KEY = 'apex_ws_session_id'

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'reconnecting'

/**
 * Maps a backend AgentStatusUpdate value to the frontend store status.
 * Backend sends snake_case enum values: "idle", "busy", "paused", "error", "offline".
 */
function mapAgentStatus(status: string): 'idle' | 'busy' | 'error' | 'paused' {
  switch (status) {
    case 'idle': return 'idle'
    case 'busy': return 'busy'
    case 'paused': return 'paused'
    case 'error':
    case 'offline': return 'error'
    default: return 'idle'
  }
}

/**
 * Maps a backend TaskStatusUpdate value to the frontend store status.
 */
function mapTaskStatus(status: string): 'pending' | 'ready' | 'running' | 'completed' | 'failed' | 'cancelled' {
  switch (status) {
    case 'pending': return 'pending'
    case 'ready': return 'ready'
    case 'running':
    case 'retrying': return 'running'
    case 'completed': return 'completed'
    case 'failed': return 'failed'
    case 'cancelled': return 'cancelled'
    default: return 'pending'
  }
}

export function useWebSocket() {
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttempts = useRef(0)
  const reconnectTimeout = useRef<NodeJS.Timeout>()

  // FIX Issue #1: Use useState instead of useRef for reactive connection state
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected')

  const { setWsConnected, setAgent, setTask, setMetrics, addApproval } = useStore()

  const handleMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const rawData = JSON.parse(event.data)

        // FIX Issue #4: Validate message using Zod before processing
        const data = parseWsMessage(rawData)

        if (!data) {
          console.warn('[WS] Invalid message received, discarding')
          return
        }

        // Process validated message
        switch (data.type) {
          // ── Connection lifecycle ──────────────────────────────────────
          case 'connected': {
            console.log('[WS] Connected to Apex')
            const sessionId = data.session_id
            if (sessionId) {
              sessionStorage.setItem(SESSION_STORAGE_KEY, sessionId)
            }
            break
          }

          case 'session_restored':
            console.log('[WS] Session restored successfully')
            break

          case 'subscribed':
            // Subscription confirmed by server
            break

          case 'heartbeat':
            // Server heartbeat — no action needed
            break

          // ── Agent updates ────────────────────────────────────────────
          case 'agent_update':
            setAgent({
              id: data.agent_id,
              name: data.name || 'Unknown',
              model: data.model || 'unknown',
              status: mapAgentStatus(data.status || 'idle'),
              currentLoad: data.current_load || 0,
              maxLoad: data.max_load || 10,
              successRate: data.success_rate || 1,
              reputationScore: data.reputation_score || 1,
              totalTokens: data.total_tokens || 0,
              totalCost: data.total_cost || 0,
              confidence: data.confidence,
            })
            break

          // ── Task updates ─────────────────────────────────────────────
          case 'task_update':
            setTask({
              id: data.task_id,
              dagId: data.dag_id || '',
              name: data.name || 'Unknown Task',
              status: mapTaskStatus(data.status || 'pending'),
              agentId: data.agent_id,
              tokensUsed: data.tokens_used || 0,
              costDollars: data.cost_dollars || 0,
              createdAt: data.created_at || data.timestamp || new Date().toISOString(),
              startedAt: data.started_at,
              completedAt: data.completed_at,
            })
            break

          // ── Metrics snapshot ─────────────────────────────────────────
          case 'metrics': {
            const agents = data.agents
            const tasks = data.tasks
            const resources = data.resources
            setMetrics({
              totalAgents: agents?.total ?? 0,
              activeAgents: agents?.active ?? 0,
              runningTasks: tasks?.running ?? 0,
              completedTasks: tasks?.completed_last_hour ?? 0,
              failedTasks: tasks?.failed_last_hour ?? 0,
              totalTokens: resources?.total_tokens_used ?? 0,
              totalCost: resources?.total_cost_dollars ?? 0,
              avgLatencyMs: tasks?.avg_duration_ms ?? 0,
              successRate: agents?.avg_success_rate ?? 0,
            })
            break
          }

          // ── Approval requests ────────────────────────────────────────
          case 'approval_required':
            addApproval({
              id: data.request_id,
              taskId: data.task_id,
              agentId: data.agent_id,
              actionType: data.approval_type,
              actionData: data.details || {},
              riskScore: 0,
              status: 'pending',
              createdAt: data.created_at || new Date().toISOString(),
            })
            // FIX Issue #6: Show toast notification for approval requests
            toast.custom((t) => (
              <div className={`bg-blue-600 text-white px-4 py-3 rounded-lg shadow-lg ${t.visible ? 'animate-enter' : 'animate-leave'}`}>
                <span className="font-medium">Approval Required</span>
                <span className="ml-2 text-blue-200">Task needs your attention</span>
              </div>
            ), { duration: 5000 })
            break

          // ── Keep-alive ───────────────────────────────────────────────
          case 'pong':
            break

          // ── Errors ───────────────────────────────────────────────────
          case 'error':
            console.error('[WS] Server error:', data.message || data.code)
            // FIX Issue #6: Show toast notification for server errors
            toast.error(data.message || 'Server error occurred', { id: 'ws-error' })
            break
        }
      } catch (error) {
        console.error('[WS] Failed to parse message:', error)
        // FIX Issue #6: Show toast notification for parse errors
        toast.error('Failed to process server message', { id: 'ws-parse-error' })
      }
    },
    [setAgent, setTask, setMetrics, addApproval]
  )

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return
    }

    const isReconnect = reconnectAttempts.current > 0
    // FIX Issue #1: Use useState setter for reactive updates
    setConnectionState(isReconnect ? 'reconnecting' : 'connecting')
    console.log('[WS] Connecting to', WS_URL, isReconnect ? '(reconnect)' : '')
    const ws = new WebSocket(WS_URL)

    ws.onopen = () => {
      console.log('[WS] Connection established')
      // FIX Issue #1: Use useState setter for reactive updates
      setConnectionState('connected')
      setWsConnected(true)
      reconnectAttempts.current = 0

      // Attempt session recovery on reconnect
      const previousSessionId = sessionStorage.getItem(SESSION_STORAGE_KEY)
      if (isReconnect && previousSessionId) {
        console.log('[WS] Attempting session recovery:', previousSessionId)
        ws.send(JSON.stringify({
          type: 'session_restore',
          session_id: previousSessionId,
        }))
      }

      // Subscribe to all updates — must match backend SubscriptionTarget enum
      ws.send(JSON.stringify({ type: 'subscribe', target: { resource: 'all_agents' } }))
      ws.send(JSON.stringify({ type: 'subscribe', target: { resource: 'all_tasks' } }))
      ws.send(JSON.stringify({ type: 'subscribe', target: { resource: 'metrics', interval_secs: 5 } }))
      ws.send(JSON.stringify({ type: 'subscribe', target: { resource: 'approvals' } }))
    }

    ws.onmessage = handleMessage

    ws.onclose = (event) => {
      console.log('[WS] Connection closed:', event.code, event.reason)
      // FIX Issue #1: Use useState setter for reactive updates
      setConnectionState('disconnected')
      setWsConnected(false)
      wsRef.current = null

      // Attempt reconnect
      if (reconnectAttempts.current < MAX_RECONNECT_ATTEMPTS) {
        reconnectAttempts.current++
        const delay = RECONNECT_DELAY * Math.min(reconnectAttempts.current, 5)
        console.log(`[WS] Reconnecting in ${delay}ms (attempt ${reconnectAttempts.current})`)
        reconnectTimeout.current = setTimeout(connect, delay)
      } else {
        console.error('[WS] Max reconnection attempts reached')
        // FIX Issue #6: Show toast notification for max reconnect attempts
        toast.error('Connection lost. Please refresh the page.', { id: 'ws-reconnect-failed' })
      }
    }

    ws.onerror = (error) => {
      console.error('[WS] Error:', error)
      // FIX Issue #6: Show toast notification for connection errors
      toast.error('WebSocket connection error', { id: 'ws-error' })
    }

    wsRef.current = ws
  }, [handleMessage, setWsConnected])

  // Send message helper
  const sendMessage = useCallback((message: { type: string; [key: string]: unknown }) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message))
    } else {
      console.warn('[WS] Cannot send message - not connected')
    }
  }, [])

  // Ping to keep connection alive
  useEffect(() => {
    const pingInterval = setInterval(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: 'ping' }))
      }
    }, 30000)

    return () => clearInterval(pingInterval)
  }, [])

  // Connect on mount
  useEffect(() => {
    connect()

    return () => {
      if (reconnectTimeout.current) {
        clearTimeout(reconnectTimeout.current)
      }
      if (wsRef.current) {
        wsRef.current.close()
      }
    }
  }, [connect])

  // FIX Issue #1: Return function to get current state for reactivity, plus connectionState for direct access
  return {
    sendMessage,
    connectionState,
    getConnectionState: () => connectionState,
  }
}
