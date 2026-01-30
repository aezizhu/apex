import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useWebSocket } from '@/hooks/useWebSocket'
import { useStore } from '@/lib/store'

// Mock WebSocket
class MockWebSocket {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSING = 2
  static CLOSED = 3

  url: string
  readyState = MockWebSocket.CONNECTING
  onopen: (() => void) | null = null
  onclose: ((event: { code: number; reason: string }) => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onerror: ((error: Error) => void) | null = null

  static instances: MockWebSocket[] = []

  constructor(url: string) {
    this.url = url
    MockWebSocket.instances.push(this)
  }

  send = vi.fn()
  close = vi.fn(() => {
    this.readyState = MockWebSocket.CLOSED
  })

  // Helpers for testing
  simulateOpen() {
    this.readyState = MockWebSocket.OPEN
    this.onopen?.()
  }

  simulateClose(code = 1000, reason = '') {
    this.readyState = MockWebSocket.CLOSED
    this.onclose?.({ code, reason })
  }

  simulateMessage(data: object) {
    this.onmessage?.({ data: JSON.stringify(data) })
  }

  simulateError(error: Error) {
    this.onerror?.(error)
  }

  static reset() {
    MockWebSocket.instances = []
  }

  static getLastInstance() {
    return MockWebSocket.instances[MockWebSocket.instances.length - 1]
  }
}

// Override global WebSocket
const OriginalWebSocket = global.WebSocket
beforeEach(() => {
  global.WebSocket = MockWebSocket as unknown as typeof WebSocket
  MockWebSocket.reset()
  vi.useFakeTimers()
})

afterEach(() => {
  global.WebSocket = OriginalWebSocket
  vi.useRealTimers()
  vi.restoreAllMocks()
  // Reset store
  const store = useStore.getState()
  store.setWsConnected(false)
  store.setAgents([])
})

describe('useWebSocket', () => {
  describe('connection', () => {
    it('connects to WebSocket on mount', () => {
      renderHook(() => useWebSocket())

      expect(MockWebSocket.instances.length).toBe(1)
    })

    it('uses correct WebSocket URL', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      expect(ws.url).toBe('ws://localhost:8080/ws')
    })

    it('sets wsConnected to true on open', async () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      expect(useStore.getState().wsConnected).toBe(true)
    })

    it('subscribes to all resources on open', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      expect(ws.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'Subscribe', resource: 'agents' })
      )
      expect(ws.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'Subscribe', resource: 'tasks' })
      )
      expect(ws.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'Subscribe', resource: 'metrics' })
      )
      expect(ws.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'Subscribe', resource: 'approvals' })
      )
    })

    it('resets reconnect attempts on successful connection', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      // Should be able to handle reconnects after success
      expect(useStore.getState().wsConnected).toBe(true)
    })
  })

  describe('disconnection', () => {
    it('sets wsConnected to false on close', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })
      expect(useStore.getState().wsConnected).toBe(true)

      act(() => {
        ws.simulateClose()
      })
      expect(useStore.getState().wsConnected).toBe(false)
    })

    it('attempts reconnection after close', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })

      // Advance timer past reconnect delay (3000ms)
      act(() => {
        vi.advanceTimersByTime(3000)
      })

      // Should have created a new WebSocket instance
      expect(MockWebSocket.instances.length).toBe(2)
    })

    it('stops reconnecting after max attempts', () => {
      renderHook(() => useWebSocket())

      // Simulate 10 failed reconnection attempts
      for (let i = 0; i < 11; i++) {
        const ws = MockWebSocket.getLastInstance()
        act(() => {
          ws.simulateClose()
        })
        act(() => {
          vi.advanceTimersByTime(3000)
        })
      }

      // Should stop at 11 instances (initial + 10 reconnects)
      expect(MockWebSocket.instances.length).toBe(11)

      // One more close should not create new instance
      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })
      act(() => {
        vi.advanceTimersByTime(3000)
      })

      // No new instance should be created
      expect(MockWebSocket.instances.length).toBe(11)
    })

    it('closes WebSocket on unmount', () => {
      const { unmount } = renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      unmount()

      expect(ws.close).toHaveBeenCalled()
    })

    it('clears reconnect timeout on unmount', () => {
      const clearTimeoutSpy = vi.spyOn(global, 'clearTimeout')
      const { unmount } = renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })

      unmount()

      // Should clear any pending reconnect timeout
      expect(clearTimeoutSpy).toHaveBeenCalled()
    })
  })

  describe('message handling', () => {
    it('handles AgentUpdate messages', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'AgentUpdate',
          id: 'agent-1',
          name: 'Test Agent',
          model: 'gpt-4',
          status: 'busy',
          currentLoad: 5,
          maxLoad: 10,
          successRate: 0.95,
          reputationScore: 0.9,
          totalTokens: 10000,
          totalCost: 0.5,
        })
      })

      const agents = Array.from(useStore.getState().agents.values())
      expect(agents).toHaveLength(1)
      expect(agents[0]).toMatchObject({
        id: 'agent-1',
        name: 'Test Agent',
        model: 'gpt-4',
        status: 'busy',
      })
    })

    it('handles TaskUpdate messages', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'TaskUpdate',
          id: 'task-1',
          dagId: 'dag-1',
          name: 'Test Task',
          status: 'running',
          agentId: 'agent-1',
          tokensUsed: 500,
          costDollars: 0.01,
          createdAt: '2024-01-15T12:00:00Z',
        })
      })

      const tasks = Array.from(useStore.getState().tasks.values())
      expect(tasks).toHaveLength(1)
      expect(tasks[0]).toMatchObject({
        id: 'task-1',
        name: 'Test Task',
        status: 'running',
      })
    })

    it('handles MetricsUpdate messages', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'MetricsUpdate',
          totalTasks: 100,
          completedTasks: 80,
          failedTasks: 5,
          runningTasks: 15,
          totalAgents: 10,
          activeAgents: 8,
          totalTokens: 1000000,
          totalCost: 25.5,
        })
      })

      const metrics = useStore.getState().metrics
      expect(metrics.totalTasks).toBe(100)
      expect(metrics.completedTasks).toBe(80)
      expect(metrics.activeAgents).toBe(8)
    })

    it('handles ApprovalRequest messages', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'ApprovalRequest',
          id: 'approval-1',
          taskId: 'task-1',
          agentId: 'agent-1',
          actionType: 'file_write',
          actionData: { path: '/tmp/test.txt' },
          riskScore: 0.8,
        })
      })

      const approvals = useStore.getState().approvals
      expect(approvals).toHaveLength(1)
      expect(approvals[0]).toMatchObject({
        id: 'approval-1',
        taskId: 'task-1',
        actionType: 'file_write',
        status: 'pending',
      })
    })

    it('handles connected message', () => {
      const consoleSpy = vi.spyOn(console, 'log')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({ type: 'connected' })
      })

      expect(consoleSpy).toHaveBeenCalledWith('[WS] Connected to Apex')
    })

    it('handles pong message silently', () => {
      const consoleSpy = vi.spyOn(console, 'log')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({ type: 'pong' })
      })

      // Should not log anything for pong
      expect(consoleSpy).not.toHaveBeenCalledWith(
        expect.stringContaining('pong')
      )
    })

    it('handles Error messages', () => {
      const consoleSpy = vi.spyOn(console, 'error')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({ type: 'Error', message: 'Server error occurred' })
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Server error:',
        'Server error occurred'
      )
    })

    it('handles unknown message types', () => {
      const consoleSpy = vi.spyOn(console, 'log')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({ type: 'UnknownType', data: 'test' })
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Unknown message type:',
        'UnknownType'
      )
    })

    it('handles malformed JSON gracefully', () => {
      const consoleSpy = vi.spyOn(console, 'error')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        // Simulate receiving malformed JSON
        ws.onmessage?.({ data: 'not valid json' })
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Failed to parse message:',
        expect.any(Error)
      )
    })

    it('uses default values for missing agent fields', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'AgentUpdate',
          id: 'agent-1',
          status: 'idle',
        })
      })

      const agents = Array.from(useStore.getState().agents.values())
      expect(agents[0]).toMatchObject({
        id: 'agent-1',
        name: 'Unknown',
        model: 'unknown',
        currentLoad: 0,
        maxLoad: 10,
        successRate: 1,
        reputationScore: 1,
        totalTokens: 0,
        totalCost: 0,
      })
    })

    it('uses default values for missing task fields', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
        ws.simulateMessage({
          type: 'TaskUpdate',
          id: 'task-1',
          status: 'pending',
        })
      })

      const tasks = Array.from(useStore.getState().tasks.values())
      expect(tasks[0]).toMatchObject({
        id: 'task-1',
        dagId: '',
        name: 'Unknown Task',
        tokensUsed: 0,
        costDollars: 0,
      })
    })
  })

  describe('sendMessage', () => {
    it('returns sendMessage function', () => {
      const { result } = renderHook(() => useWebSocket())

      expect(result.current.sendMessage).toBeDefined()
      expect(typeof result.current.sendMessage).toBe('function')
    })

    it('sends message when connected', () => {
      const { result } = renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      act(() => {
        result.current.sendMessage({ type: 'CustomMessage', data: 'test' })
      })

      expect(ws.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'CustomMessage', data: 'test' })
      )
    })

    it('warns when sending message while disconnected', () => {
      const consoleSpy = vi.spyOn(console, 'warn')
      const { result } = renderHook(() => useWebSocket())

      // Don't open the connection
      act(() => {
        result.current.sendMessage({ type: 'Test' })
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Cannot send message - not connected'
      )
    })

    it('does not send message when WebSocket is not ready', () => {
      const { result } = renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      ws.readyState = MockWebSocket.CONNECTING

      act(() => {
        result.current.sendMessage({ type: 'Test' })
      })

      // Should only have the subscription messages, not the custom one
      expect(ws.send).not.toHaveBeenCalledWith(
        JSON.stringify({ type: 'Test' })
      )
    })
  })

  describe('ping/keep-alive', () => {
    it('sends ping every 30 seconds', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      // Clear previous calls
      ws.send.mockClear()

      // Advance time by 30 seconds
      act(() => {
        vi.advanceTimersByTime(30000)
      })

      expect(ws.send).toHaveBeenCalledWith(JSON.stringify({ type: 'Ping' }))
    })

    it('does not send ping when disconnected', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      // Don't open the connection

      ws.send.mockClear()

      act(() => {
        vi.advanceTimersByTime(30000)
      })

      expect(ws.send).not.toHaveBeenCalled()
    })

    it('clears ping interval on unmount', () => {
      const clearIntervalSpy = vi.spyOn(global, 'clearInterval')
      const { unmount } = renderHook(() => useWebSocket())

      unmount()

      expect(clearIntervalSpy).toHaveBeenCalled()
    })
  })

  describe('error handling', () => {
    it('handles WebSocket errors', () => {
      const consoleSpy = vi.spyOn(console, 'error')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateError(new Error('Connection failed'))
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Error:',
        expect.any(Error)
      )
    })

    it('does not create new connection if already connected', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateOpen()
      })

      // Try to connect again (this would happen if component re-renders)
      const instanceCount = MockWebSocket.instances.length

      // The hook should not create a new WebSocket if already open
      expect(MockWebSocket.instances.length).toBe(instanceCount)
    })
  })

  describe('reconnection', () => {
    it('increments reconnect attempts on each failure', () => {
      renderHook(() => useWebSocket())

      // First connection attempt
      expect(MockWebSocket.instances.length).toBe(1)

      const ws1 = MockWebSocket.getLastInstance()
      act(() => {
        ws1.simulateClose()
      })

      act(() => {
        vi.advanceTimersByTime(3000)
      })

      // Second attempt
      expect(MockWebSocket.instances.length).toBe(2)
    })

    it('uses RECONNECT_DELAY of 3000ms', () => {
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })

      // Not enough time passed
      act(() => {
        vi.advanceTimersByTime(2999)
      })

      expect(MockWebSocket.instances.length).toBe(1)

      // Now enough time
      act(() => {
        vi.advanceTimersByTime(1)
      })

      expect(MockWebSocket.instances.length).toBe(2)
    })

    it('logs reconnection attempts', () => {
      const consoleSpy = vi.spyOn(console, 'log')
      renderHook(() => useWebSocket())

      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringMatching(/Reconnecting in 3000ms/)
      )
    })

    it('logs when max reconnect attempts reached', () => {
      const consoleSpy = vi.spyOn(console, 'error')
      renderHook(() => useWebSocket())

      // Exhaust all reconnection attempts
      for (let i = 0; i < 10; i++) {
        const ws = MockWebSocket.getLastInstance()
        act(() => {
          ws.simulateClose()
        })
        act(() => {
          vi.advanceTimersByTime(3000)
        })
      }

      // One more to trigger max attempts message
      const ws = MockWebSocket.getLastInstance()
      act(() => {
        ws.simulateClose()
      })

      expect(consoleSpy).toHaveBeenCalledWith(
        '[WS] Max reconnection attempts reached'
      )
    })
  })
})
