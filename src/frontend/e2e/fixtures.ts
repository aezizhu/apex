import { test as base, expect, Page } from '@playwright/test'

// ═══════════════════════════════════════════════════════════════════════════════
// Types for test data
// ═══════════════════════════════════════════════════════════════════════════════

export interface MockAgent {
  id: string
  name: string
  model: string
  status: 'idle' | 'busy' | 'error' | 'paused'
  currentLoad: number
  maxLoad: number
  successRate: number
  reputationScore: number
  totalTokens: number
  totalCost: number
  confidence?: number
}

export interface MockTask {
  id: string
  dagId: string
  name: string
  status: 'pending' | 'ready' | 'running' | 'completed' | 'failed' | 'cancelled'
  agentId?: string
  tokensUsed: number
  costDollars: number
  createdAt: string
  startedAt?: string
  completedAt?: string
}

export interface MockApproval {
  id: string
  taskId: string
  agentId: string
  actionType: string
  actionData: Record<string, unknown>
  riskScore: number
  status: 'pending' | 'approved' | 'denied' | 'expired'
  createdAt: string
}

export interface MockMetrics {
  totalTasks: number
  completedTasks: number
  failedTasks: number
  runningTasks: number
  totalAgents: number
  activeAgents: number
  totalTokens: number
  totalCost: number
  avgLatencyMs: number
  successRate: number
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test data generators
// ═══════════════════════════════════════════════════════════════════════════════

export function createMockAgent(overrides: Partial<MockAgent> = {}): MockAgent {
  return {
    id: `agent-${Math.random().toString(36).substring(7)}`,
    name: `Agent ${Math.floor(Math.random() * 1000)}`,
    model: 'gpt-4',
    status: 'idle',
    currentLoad: 0,
    maxLoad: 10,
    successRate: 0.95,
    reputationScore: 0.9,
    totalTokens: 50000,
    totalCost: 2.5,
    confidence: 0.85,
    ...overrides,
  }
}

export function createMockTask(overrides: Partial<MockTask> = {}): MockTask {
  const now = new Date()
  return {
    id: `task-${Math.random().toString(36).substring(7)}`,
    dagId: `dag-${Math.random().toString(36).substring(7)}`,
    name: `Task ${Math.floor(Math.random() * 1000)}`,
    status: 'pending',
    tokensUsed: 1000,
    costDollars: 0.05,
    createdAt: now.toISOString(),
    ...overrides,
  }
}

export function createMockApproval(overrides: Partial<MockApproval> = {}): MockApproval {
  return {
    id: `approval-${Math.random().toString(36).substring(7)}`,
    taskId: `task-${Math.random().toString(36).substring(7)}`,
    agentId: `agent-${Math.random().toString(36).substring(7)}`,
    actionType: 'file_write',
    actionData: { path: '/tmp/test.txt', content: 'test content' },
    riskScore: 0.5,
    status: 'pending',
    createdAt: new Date().toISOString(),
    ...overrides,
  }
}

export function createMockMetrics(overrides: Partial<MockMetrics> = {}): MockMetrics {
  return {
    totalTasks: 100,
    completedTasks: 85,
    failedTasks: 5,
    runningTasks: 10,
    totalAgents: 50,
    activeAgents: 25,
    totalTokens: 5000000,
    totalCost: 250.0,
    avgLatencyMs: 150,
    successRate: 0.95,
    ...overrides,
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WebSocket mock helper
// ═══════════════════════════════════════════════════════════════════════════════

export class WebSocketMock {
  private page: Page
  private messageQueue: Array<{ type: string; [key: string]: unknown }> = []

  constructor(page: Page) {
    this.page = page
  }

  async setup() {
    await this.page.addInitScript(() => {
      // Store original WebSocket
      const OriginalWebSocket = window.WebSocket

      // Mock WebSocket class
      class MockWebSocket extends EventTarget {
        static CONNECTING = 0
        static OPEN = 1
        static CLOSING = 2
        static CLOSED = 3

        readyState = MockWebSocket.OPEN
        url: string
        protocol = ''
        extensions = ''
        bufferedAmount = 0
        binaryType: BinaryType = 'blob'

        onopen: ((this: WebSocket, ev: Event) => void) | null = null
        onclose: ((this: WebSocket, ev: CloseEvent) => void) | null = null
        onmessage: ((this: WebSocket, ev: MessageEvent) => void) | null = null
        onerror: ((this: WebSocket, ev: Event) => void) | null = null

        constructor(url: string | URL, protocols?: string | string[]) {
          super()
          this.url = typeof url === 'string' ? url : url.toString()

          // Store reference for external access
          ;(window as unknown as { __mockWs: MockWebSocket }).__mockWs = this

          // Simulate connection
          setTimeout(() => {
            this.readyState = MockWebSocket.OPEN
            const event = new Event('open')
            this.dispatchEvent(event)
            this.onopen?.call(this as unknown as WebSocket, event)

            // Send connected message
            const connectedMsg = new MessageEvent('message', {
              data: JSON.stringify({ type: 'connected' }),
            })
            this.dispatchEvent(connectedMsg)
            this.onmessage?.call(this as unknown as WebSocket, connectedMsg)
          }, 10)
        }

        send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
          // Emit custom event for test inspection
          const event = new CustomEvent('ws-send', { detail: data })
          window.dispatchEvent(event)
        }

        close(code?: number, reason?: string): void {
          this.readyState = MockWebSocket.CLOSED
          const event = new CloseEvent('close', { code, reason })
          this.dispatchEvent(event)
          this.onclose?.call(this as unknown as WebSocket, event)
        }

        // Helper to receive mock messages
        receiveMessage(data: unknown): void {
          const event = new MessageEvent('message', {
            data: JSON.stringify(data),
          })
          this.dispatchEvent(event)
          this.onmessage?.call(this as unknown as WebSocket, event)
        }
      }

      // Replace WebSocket globally
      ;(window as unknown as { WebSocket: typeof MockWebSocket }).WebSocket = MockWebSocket
    })
  }

  async sendMessage(message: { type: string; [key: string]: unknown }) {
    await this.page.evaluate((msg) => {
      const ws = (window as unknown as { __mockWs?: { receiveMessage: (data: unknown) => void } })
        .__mockWs
      if (ws) {
        ws.receiveMessage(msg)
      }
    }, message)
  }

  async sendAgentUpdate(agent: MockAgent) {
    await this.sendMessage({ type: 'AgentUpdate', ...agent })
  }

  async sendTaskUpdate(task: MockTask) {
    await this.sendMessage({ type: 'TaskUpdate', ...task })
  }

  async sendMetricsUpdate(metrics: MockMetrics) {
    await this.sendMessage({ type: 'MetricsUpdate', ...metrics })
  }

  async sendApprovalRequest(approval: MockApproval) {
    await this.sendMessage({ type: 'ApprovalRequest', ...approval })
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Extended test fixture with WebSocket mock
// ═══════════════════════════════════════════════════════════════════════════════

interface TestFixtures {
  wsMock: WebSocketMock
}

export const test = base.extend<TestFixtures>({
  wsMock: async ({ page }, use) => {
    const wsMock = new WebSocketMock(page)
    await wsMock.setup()
    await use(wsMock)
  },
})

export { expect }
