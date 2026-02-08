import { useEffect, useRef, useCallback } from 'react'
import { useStore } from '../lib/store'

const WS_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws'
const RECONNECT_DELAY = 3000
const MAX_RECONNECT_ATTEMPTS = 10
const SESSION_STORAGE_KEY = 'apex_ws_session_id'

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'reconnecting'

interface WsMessage {
  type: string
  [key: string]: unknown
}

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
  const connectionState = useRef<ConnectionState>('disconnected')

  const { setWsConnected, setAgent, setTask, setMetrics, addApproval } = useStore()

  const handleMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data) as WsMessage

        switch (data.type) {
          // ── Connection lifecycle ──────────────────────────────────────
          case 'connected': {
            console.log('[WS] Connected to Apex')
            const sessionId = data.session_id as string | undefined
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
          // Backend serializes ServerMessage::AgentUpdate as "agent_update"
          case 'agent_update':
            setAgent({
              id: data.agent_id as string,
              name: data.name as string || 'Unknown',
              model: (data.model as string) || 'unknown',
              status: mapAgentStatus(data.status as string),
              currentLoad: (data.current_load as number) || 0,
              maxLoad: (data.max_load as number) || 10,
              successRate: (data.success_rate as number) || 1,
              reputationScore: (data.reputation_score as number) || 1,
              totalTokens: (data.total_tokens as number) || 0,
              totalCost: (data.total_cost as number) || 0,
              confidence: data.confidence as number | undefined,
            })
            break

          // ── Task updates ─────────────────────────────────────────────
          case 'task_update':
            setTask({
              id: data.task_id as string,
              dagId: (data.dag_id as string) || '',
              name: (data.name as string) || 'Unknown Task',
              status: mapTaskStatus(data.status as string),
              agentId: data.agent_id as string | undefined,
              tokensUsed: (data.tokens_used as number) || 0,
              costDollars: (data.cost_dollars as number) || 0,
              createdAt: (data.created_at as string) || (data.timestamp as string) || new Date().toISOString(),
              startedAt: data.started_at as string | undefined,
              completedAt: data.completed_at as string | undefined,
            })
            break

          // ── Metrics snapshot ─────────────────────────────────────────
          // Backend sends ServerMessage::Metrics with nested agents/tasks/resources
          case 'metrics': {
            const agents = data.agents as Record<string, number> | undefined
            const tasks = data.tasks as Record<string, number> | undefined
            const resources = data.resources as Record<string, number> | undefined
            setMetrics({
              totalAgents: agents?.total as number ?? 0,
              activeAgents: agents?.active as number ?? 0,
              runningTasks: tasks?.running as number ?? 0,
              completedTasks: tasks?.completed_last_hour as number ?? 0,
              failedTasks: tasks?.failed_last_hour as number ?? 0,
              totalTokens: resources?.total_tokens_used as number ?? 0,
              totalCost: resources?.total_cost_dollars as number ?? 0,
              avgLatencyMs: tasks?.avg_duration_ms as number ?? 0,
              successRate: agents?.avg_success_rate as number ?? 0,
            })
            break
          }

          // ── Approval requests ────────────────────────────────────────
          case 'approval_required':
            addApproval({
              id: data.request_id as string,
              taskId: data.task_id as string,
              agentId: data.agent_id as string,
              actionType: data.approval_type as string,
              actionData: data.details as Record<string, unknown> || {},
              riskScore: 0,
              status: 'pending',
              createdAt: (data.created_at as string) || new Date().toISOString(),
            })
            break

          // ── Keep-alive ───────────────────────────────────────────────
          case 'pong':
            break

          // ── Errors ───────────────────────────────────────────────────
          case 'error':
            console.error('[WS] Server error:', data.message || data.code)
            break

          default:
            console.log('[WS] Unknown message type:', data.type)
        }
      } catch (error) {
        console.error('[WS] Failed to parse message:', error)
      }
    },
    [setAgent, setTask, setMetrics, addApproval]
  )

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return
    }

    const isReconnect = reconnectAttempts.current > 0
    connectionState.current = isReconnect ? 'reconnecting' : 'connecting'
    console.log('[WS] Connecting to', WS_URL, isReconnect ? '(reconnect)' : '')
    const ws = new WebSocket(WS_URL)

    ws.onopen = () => {
      console.log('[WS] Connection established')
      connectionState.current = 'connected'
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
      connectionState.current = 'disconnected'
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
      }
    }

    ws.onerror = (error) => {
      console.error('[WS] Error:', error)
    }

    wsRef.current = ws
  }, [handleMessage, setWsConnected])

  // Send message helper
  const sendMessage = useCallback((message: WsMessage) => {
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

  return { sendMessage, connectionState: connectionState.current }
}
