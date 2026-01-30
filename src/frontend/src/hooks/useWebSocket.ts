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
          case 'connected': {
            console.log('[WS] Connected to Apex')
            // Store session ID for recovery on reconnect
            const sessionId = data.session_id as string | undefined
            if (sessionId) {
              sessionStorage.setItem(SESSION_STORAGE_KEY, sessionId)
            }
            break
          }

          case 'session_restored':
            console.log('[WS] Session restored successfully')
            break

          case 'AgentUpdate':
            setAgent({
              id: data.id as string,
              name: data.name as string || 'Unknown',
              model: data.model as string || 'unknown',
              status: data.status as 'idle' | 'busy' | 'error' | 'paused',
              currentLoad: data.currentLoad as number || 0,
              maxLoad: data.maxLoad as number || 10,
              successRate: data.successRate as number || 1,
              reputationScore: data.reputationScore as number || 1,
              totalTokens: data.totalTokens as number || 0,
              totalCost: data.totalCost as number || 0,
              confidence: data.confidence as number | undefined,
            })
            break

          case 'TaskUpdate':
            setTask({
              id: data.id as string,
              dagId: data.dagId as string || '',
              name: data.name as string || 'Unknown Task',
              status: data.status as 'pending' | 'ready' | 'running' | 'completed' | 'failed' | 'cancelled',
              agentId: data.agentId as string | undefined,
              tokensUsed: data.tokensUsed as number || 0,
              costDollars: data.costDollars as number || 0,
              createdAt: data.createdAt as string || new Date().toISOString(),
              startedAt: data.startedAt as string | undefined,
              completedAt: data.completedAt as string | undefined,
            })
            break

          case 'MetricsUpdate':
            setMetrics({
              totalTasks: data.totalTasks as number,
              completedTasks: data.completedTasks as number,
              failedTasks: data.failedTasks as number,
              runningTasks: data.runningTasks as number,
              totalAgents: data.totalAgents as number,
              activeAgents: data.activeAgents as number,
              totalTokens: data.totalTokens as number,
              totalCost: data.totalCost as number,
            })
            break

          case 'ApprovalRequest':
            addApproval({
              id: data.id as string,
              taskId: data.taskId as string,
              agentId: data.agentId as string,
              actionType: data.actionType as string,
              actionData: data.actionData as Record<string, unknown>,
              riskScore: data.riskScore as number,
              status: 'pending',
              createdAt: new Date().toISOString(),
            })
            break

          case 'pong':
            // Keep-alive response
            break

          case 'Error':
            console.error('[WS] Server error:', data.message)
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
          type: 'SessionRestore',
          session_id: previousSessionId,
        }))
      }

      // Subscribe to all updates
      ws.send(JSON.stringify({ type: 'Subscribe', resource: 'agents' }))
      ws.send(JSON.stringify({ type: 'Subscribe', resource: 'tasks' }))
      ws.send(JSON.stringify({ type: 'Subscribe', resource: 'metrics' }))
      ws.send(JSON.stringify({ type: 'Subscribe', resource: 'approvals' }))
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
        wsRef.current.send(JSON.stringify({ type: 'Ping' }))
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
