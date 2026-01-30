/**
 * Tests for ApexWebSocket
 */

import WebSocket from 'ws';
import { EventEmitter } from 'events';
import {
  ApexWebSocket,
  createApexWebSocket,
  WebSocketEventType,
  WebSocketError,
  TaskStatus,
  TaskPriority,
  AgentStatus,
  DAGStatus,
  ApprovalStatus,
} from '../src';

// Mock the ws module
jest.mock('ws');

const MockWebSocket = WebSocket as jest.MockedClass<typeof WebSocket>;

describe('ApexWebSocket', () => {
  let ws: ApexWebSocket;
  let mockWsInstance: jest.Mocked<WebSocket> & EventEmitter;

  const baseConfig = {
    url: 'wss://api.apex.test/ws',
    apiKey: 'test-api-key',
    autoReconnect: true,
    reconnectInterval: 100,
    maxReconnectAttempts: 3,
    heartbeatInterval: 1000,
    heartbeatTimeout: 500,
  };

  beforeEach(() => {
    jest.useFakeTimers();

    // Create mock WebSocket instance
    mockWsInstance = new EventEmitter() as jest.Mocked<WebSocket> & EventEmitter;
    mockWsInstance.readyState = WebSocket.CONNECTING;
    mockWsInstance.send = jest.fn();
    mockWsInstance.close = jest.fn();

    MockWebSocket.mockImplementation(() => mockWsInstance as unknown as WebSocket);

    ws = new ApexWebSocket(baseConfig);
  });

  afterEach(() => {
    jest.clearAllTimers();
    jest.useRealTimers();
    jest.clearAllMocks();
  });

  describe('constructor', () => {
    it('should create WebSocket client with default configuration', () => {
      const minimalWs = new ApexWebSocket({
        url: 'wss://api.apex.test/ws',
      });
      expect(minimalWs).toBeInstanceOf(ApexWebSocket);
    });

    it('should create WebSocket client with custom configuration', () => {
      expect(ws).toBeInstanceOf(ApexWebSocket);
    });
  });

  describe('createApexWebSocket factory', () => {
    it('should create a new ApexWebSocket instance', () => {
      const factoryWs = createApexWebSocket(baseConfig);
      expect(factoryWs).toBeInstanceOf(ApexWebSocket);
    });
  });

  describe('connect', () => {
    it('should connect to WebSocket server', async () => {
      const connectPromise = ws.connect();

      // Simulate successful connection
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');

      await expect(connectPromise).resolves.toBeUndefined();
      expect(ws.isConnected()).toBe(true);
    });

    it('should include API key in URL', async () => {
      ws.connect();

      expect(MockWebSocket).toHaveBeenCalledWith(
        expect.stringContaining('apiKey=test-api-key')
      );
    });

    it('should resolve immediately if already connected', async () => {
      const connectPromise1 = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise1;

      const connectPromise2 = ws.connect();
      await expect(connectPromise2).resolves.toBeUndefined();
    });

    it('should reject on connection error', async () => {
      const connectPromise = ws.connect();

      mockWsInstance.readyState = WebSocket.CLOSED;
      mockWsInstance.emit('error', { message: 'Connection failed' });

      await expect(connectPromise).rejects.toThrow(WebSocketError);
    });

    it('should emit open event on successful connection', async () => {
      const openHandler = jest.fn();
      ws.on('open', openHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      expect(openHandler).toHaveBeenCalled();
    });
  });

  describe('disconnect', () => {
    it('should close WebSocket connection', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      ws.disconnect();

      expect(mockWsInstance.close).toHaveBeenCalledWith(1000, 'Client disconnecting');
      expect(ws.isConnected()).toBe(false);
    });

    it('should emit close event', async () => {
      const closeHandler = jest.fn();
      ws.on('close', closeHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      ws.disconnect();
      mockWsInstance.emit('close', { code: 1000, reason: 'Client disconnecting' });

      expect(closeHandler).toHaveBeenCalledWith(1000, 'Client disconnecting');
    });

    it('should not attempt reconnect after disconnect', async () => {
      const reconnectingHandler = jest.fn();
      ws.on('reconnecting', reconnectingHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      ws.disconnect();
      mockWsInstance.emit('close', { code: 1000, reason: 'Client disconnecting' });

      jest.advanceTimersByTime(1000);

      expect(reconnectingHandler).not.toHaveBeenCalled();
    });
  });

  describe('isConnected', () => {
    it('should return false when not connected', () => {
      expect(ws.isConnected()).toBe(false);
    });

    it('should return true when connected', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      expect(ws.isConnected()).toBe(true);
    });
  });

  describe('getConnectionId', () => {
    it('should return null when not connected', () => {
      expect(ws.getConnectionId()).toBeNull();
    });

    it('should return connection ID after heartbeat', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Simulate heartbeat message
      const heartbeatMessage = JSON.stringify({
        type: WebSocketEventType.HEARTBEAT,
        payload: {
          serverTime: '2024-01-01T00:00:00Z',
          connectionId: 'conn-123',
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', heartbeatMessage);

      expect(ws.getConnectionId()).toBe('conn-123');
    });
  });

  describe('subscribe', () => {
    it('should subscribe to events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribe({
        event: WebSocketEventType.TASK_CREATED,
      });

      expect(subscriptionId).toMatch(/^sub_\d+_[a-z0-9]+$/);
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"type":"subscribe"')
      );
    });

    it('should subscribe to multiple events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribe({
        event: [
          WebSocketEventType.TASK_CREATED,
          WebSocketEventType.TASK_COMPLETED,
        ],
      });

      expect(subscriptionId).toBeDefined();
    });

    it('should subscribe with filters', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribe({
        event: WebSocketEventType.TASK_UPDATED,
        filter: { taskId: 'task-123' },
      });

      expect(subscriptionId).toBeDefined();
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"taskId":"task-123"')
      );
    });

    it('should queue subscription if not connected', () => {
      const subscriptionId = ws.subscribe({
        event: WebSocketEventType.TASK_CREATED,
      });

      expect(subscriptionId).toBeDefined();
      expect(mockWsInstance.send).not.toHaveBeenCalled();
    });

    it('should send queued subscriptions on connect', async () => {
      const subscriptionId = ws.subscribe({
        event: WebSocketEventType.TASK_CREATED,
      });

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining(subscriptionId)
      );
    });
  });

  describe('unsubscribe', () => {
    it('should unsubscribe from events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribe({
        event: WebSocketEventType.TASK_CREATED,
      });

      ws.unsubscribe(subscriptionId);

      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"type":"unsubscribe"')
      );
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining(subscriptionId)
      );
    });
  });

  describe('subscribeToTask', () => {
    it('should subscribe to task events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribeToTask('task-123');

      expect(subscriptionId).toBeDefined();
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"taskId":"task-123"')
      );
    });
  });

  describe('subscribeToAgent', () => {
    it('should subscribe to agent events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribeToAgent('agent-123');

      expect(subscriptionId).toBeDefined();
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"agentId":"agent-123"')
      );
    });
  });

  describe('subscribeToDAG', () => {
    it('should subscribe to DAG events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribeToDAG('dag-123');

      expect(subscriptionId).toBeDefined();
      expect(mockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"dagId":"dag-123"')
      );
    });
  });

  describe('subscribeToApprovals', () => {
    it('should subscribe to approval events', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const subscriptionId = ws.subscribeToApprovals();

      expect(subscriptionId).toBeDefined();
    });
  });

  describe('message handling', () => {
    it('should emit message event for all messages', async () => {
      const messageHandler = jest.fn();
      ws.on('message', messageHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_CREATED,
        payload: { task: { id: 'task-123' } },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(messageHandler).toHaveBeenCalledWith(
        expect.objectContaining({
          type: WebSocketEventType.TASK_CREATED,
        })
      );
    });

    it('should emit task.created event', async () => {
      const taskCreatedHandler = jest.fn();
      ws.on(WebSocketEventType.TASK_CREATED, taskCreatedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const task = {
        id: 'task-123',
        name: 'Test Task',
        status: TaskStatus.PENDING,
        priority: TaskPriority.NORMAL,
        retryCount: 0,
        maxRetries: 3,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_CREATED,
        payload: { task },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(taskCreatedHandler).toHaveBeenCalledWith({ task });
    });

    it('should emit task.updated event', async () => {
      const taskUpdatedHandler = jest.fn();
      ws.on(WebSocketEventType.TASK_UPDATED, taskUpdatedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_UPDATED,
        payload: {
          task: { id: 'task-123', status: TaskStatus.RUNNING },
          changes: { status: TaskStatus.RUNNING },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(taskUpdatedHandler).toHaveBeenCalled();
    });

    it('should emit task.completed event', async () => {
      const taskCompletedHandler = jest.fn();
      ws.on(WebSocketEventType.TASK_COMPLETED, taskCompletedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_COMPLETED,
        payload: {
          task: { id: 'task-123', status: TaskStatus.COMPLETED },
          duration: 5000,
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(taskCompletedHandler).toHaveBeenCalledWith(
        expect.objectContaining({ duration: 5000 })
      );
    });

    it('should emit task.failed event', async () => {
      const taskFailedHandler = jest.fn();
      ws.on(WebSocketEventType.TASK_FAILED, taskFailedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_FAILED,
        payload: {
          task: { id: 'task-123', status: TaskStatus.FAILED },
          error: { code: 'EXECUTION_ERROR', message: 'Task failed' },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(taskFailedHandler).toHaveBeenCalledWith(
        expect.objectContaining({
          error: expect.objectContaining({ code: 'EXECUTION_ERROR' }),
        })
      );
    });

    it('should emit agent.status_changed event', async () => {
      const agentStatusHandler = jest.fn();
      ws.on(WebSocketEventType.AGENT_STATUS_CHANGED, agentStatusHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.AGENT_STATUS_CHANGED,
        payload: {
          agent: { id: 'agent-123', status: AgentStatus.BUSY },
          previousStatus: AgentStatus.IDLE,
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(agentStatusHandler).toHaveBeenCalledWith(
        expect.objectContaining({ previousStatus: AgentStatus.IDLE })
      );
    });

    it('should emit dag.started event', async () => {
      const dagStartedHandler = jest.fn();
      ws.on(WebSocketEventType.DAG_STARTED, dagStartedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.DAG_STARTED,
        payload: {
          dag: { id: 'dag-123', status: DAGStatus.RUNNING },
          execution: { id: 'exec-123', status: DAGStatus.RUNNING },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(dagStartedHandler).toHaveBeenCalled();
    });

    it('should emit dag.completed event', async () => {
      const dagCompletedHandler = jest.fn();
      ws.on(WebSocketEventType.DAG_COMPLETED, dagCompletedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.DAG_COMPLETED,
        payload: {
          dag: { id: 'dag-123', status: DAGStatus.COMPLETED },
          execution: { id: 'exec-123', status: DAGStatus.COMPLETED },
          duration: 10000,
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(dagCompletedHandler).toHaveBeenCalledWith(
        expect.objectContaining({ duration: 10000 })
      );
    });

    it('should emit dag.failed event', async () => {
      const dagFailedHandler = jest.fn();
      ws.on(WebSocketEventType.DAG_FAILED, dagFailedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.DAG_FAILED,
        payload: {
          dag: { id: 'dag-123', status: DAGStatus.FAILED },
          execution: { id: 'exec-123', status: DAGStatus.FAILED },
          error: { code: 'NODE_ERROR', message: 'Node failed' },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(dagFailedHandler).toHaveBeenCalled();
    });

    it('should emit approval.required event', async () => {
      const approvalRequiredHandler = jest.fn();
      ws.on(WebSocketEventType.APPROVAL_REQUIRED, approvalRequiredHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.APPROVAL_REQUIRED,
        payload: {
          approval: {
            id: 'approval-123',
            status: ApprovalStatus.PENDING,
            approvers: ['user-1'],
          },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(approvalRequiredHandler).toHaveBeenCalled();
    });

    it('should emit approval.resolved event', async () => {
      const approvalResolvedHandler = jest.fn();
      ws.on(WebSocketEventType.APPROVAL_RESOLVED, approvalResolvedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.APPROVAL_RESOLVED,
        payload: {
          approval: { id: 'approval-123', status: ApprovalStatus.APPROVED },
          decision: 'approved',
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(approvalResolvedHandler).toHaveBeenCalledWith(
        expect.objectContaining({ decision: 'approved' })
      );
    });

    it('should emit log.message event', async () => {
      const logMessageHandler = jest.fn();
      ws.on(WebSocketEventType.LOG_MESSAGE, logMessageHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.LOG_MESSAGE,
        payload: {
          log: {
            id: 'log-1',
            taskId: 'task-123',
            level: 'info',
            message: 'Processing started',
            timestamp: '2024-01-01T00:00:00Z',
          },
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(logMessageHandler).toHaveBeenCalled();
    });

    it('should handle heartbeat event and set connection ID', async () => {
      const heartbeatHandler = jest.fn();
      ws.on(WebSocketEventType.HEARTBEAT, heartbeatHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.HEARTBEAT,
        payload: {
          serverTime: '2024-01-01T00:00:00Z',
          connectionId: 'conn-456',
        },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(heartbeatHandler).toHaveBeenCalled();
      expect(ws.getConnectionId()).toBe('conn-456');
    });

    it('should emit error on invalid JSON message', async () => {
      const errorHandler = jest.fn();
      ws.on('error', errorHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      mockWsInstance.emit('message', 'invalid json {{{');

      expect(errorHandler).toHaveBeenCalledWith(
        expect.objectContaining({
          message: expect.stringContaining('Failed to parse message'),
        })
      );
    });
  });

  describe('waitFor', () => {
    it('should wait for specific event', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const waitPromise = ws.waitFor<{ task: { id: string } }>(
        WebSocketEventType.TASK_COMPLETED
      );

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_COMPLETED,
        payload: { task: { id: 'task-123' }, duration: 5000 },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      const result = await waitPromise;
      expect(result.task.id).toBe('task-123');
    });

    it('should filter events with predicate', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const waitPromise = ws.waitFor<{ task: { id: string } }>(
        WebSocketEventType.TASK_COMPLETED,
        (payload) => payload.task.id === 'task-456'
      );

      // First message should not match
      const message1 = JSON.stringify({
        type: WebSocketEventType.TASK_COMPLETED,
        payload: { task: { id: 'task-123' }, duration: 5000 },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message1);

      // Second message should match
      const message2 = JSON.stringify({
        type: WebSocketEventType.TASK_COMPLETED,
        payload: { task: { id: 'task-456' }, duration: 3000 },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message2);

      const result = await waitPromise;
      expect(result.task.id).toBe('task-456');
    });

    it('should timeout if event not received', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const waitPromise = ws.waitFor(
        WebSocketEventType.TASK_COMPLETED,
        undefined,
        100
      );

      jest.advanceTimersByTime(150);

      await expect(waitPromise).rejects.toThrow(WebSocketError);
      await expect(waitPromise).rejects.toThrow('Timeout waiting for event');
    });
  });

  describe('event listeners', () => {
    it('should support on method', async () => {
      const handler = jest.fn();
      ws.on('open', handler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      expect(handler).toHaveBeenCalled();
    });

    it('should support once method', async () => {
      const handler = jest.fn();
      ws.once('message', handler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message1 = JSON.stringify({
        type: WebSocketEventType.TASK_CREATED,
        payload: {},
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message1);
      mockWsInstance.emit('message', message1);

      expect(handler).toHaveBeenCalledTimes(1);
    });

    it('should support off method', async () => {
      const handler = jest.fn();
      ws.on('message', handler);
      ws.off('message', handler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const message = JSON.stringify({
        type: WebSocketEventType.TASK_CREATED,
        payload: {},
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      expect(handler).not.toHaveBeenCalled();
    });
  });

  describe('reconnection', () => {
    it('should attempt reconnect on unexpected disconnect', async () => {
      const reconnectingHandler = jest.fn();
      ws.on('reconnecting', reconnectingHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Simulate unexpected disconnect
      mockWsInstance.emit('close', { code: 1006, reason: 'Connection lost' });

      jest.advanceTimersByTime(100);

      expect(reconnectingHandler).toHaveBeenCalledWith(1);
    });

    it('should use exponential backoff for reconnection', async () => {
      const reconnectingHandler = jest.fn();
      ws.on('reconnecting', reconnectingHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Simulate unexpected disconnect
      mockWsInstance.emit('close', { code: 1006, reason: 'Connection lost' });

      // First reconnect attempt
      jest.advanceTimersByTime(200);
      expect(reconnectingHandler).toHaveBeenCalledWith(1);

      // Simulate failed reconnect
      MockWebSocket.mockClear();
      mockWsInstance = new EventEmitter() as jest.Mocked<WebSocket> & EventEmitter;
      mockWsInstance.readyState = WebSocket.CONNECTING;
      mockWsInstance.send = jest.fn();
      mockWsInstance.close = jest.fn();
      MockWebSocket.mockImplementation(() => mockWsInstance as unknown as WebSocket);

      mockWsInstance.readyState = WebSocket.CLOSED;
      mockWsInstance.emit('error', { message: 'Connection failed' });
      mockWsInstance.emit('close', { code: 1006, reason: 'Connection failed' });

      // Second reconnect attempt with longer delay
      jest.advanceTimersByTime(300);
      expect(reconnectingHandler).toHaveBeenCalledWith(2);
    });

    it('should stop reconnecting after max attempts', async () => {
      const reconnectingHandler = jest.fn();
      const errorHandler = jest.fn();
      ws.on('reconnecting', reconnectingHandler);
      ws.on('error', errorHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Simulate disconnects and failed reconnects
      for (let i = 0; i < 4; i++) {
        mockWsInstance.emit('close', { code: 1006, reason: 'Connection lost' });
        jest.advanceTimersByTime(5000);

        if (i < 3) {
          MockWebSocket.mockClear();
          mockWsInstance = new EventEmitter() as jest.Mocked<WebSocket> & EventEmitter;
          mockWsInstance.readyState = WebSocket.CLOSED;
          mockWsInstance.send = jest.fn();
          mockWsInstance.close = jest.fn();
          MockWebSocket.mockImplementation(() => mockWsInstance as unknown as WebSocket);
          mockWsInstance.emit('error', { message: 'Connection failed' });
        }
      }

      expect(reconnectingHandler).toHaveBeenCalledTimes(3);
      expect(errorHandler).toHaveBeenCalledWith(
        expect.objectContaining({
          message: expect.stringContaining('Max reconnection attempts'),
        })
      );
    });

    it('should emit reconnected event on successful reconnect', async () => {
      const reconnectedHandler = jest.fn();
      ws.on('reconnected', reconnectedHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Simulate unexpected disconnect
      mockWsInstance.emit('close', { code: 1006, reason: 'Connection lost' });

      // Advance timer to trigger reconnect
      jest.advanceTimersByTime(200);

      // Create new mock for reconnection
      MockWebSocket.mockClear();
      const newMockWsInstance = new EventEmitter() as jest.Mocked<WebSocket> & EventEmitter;
      newMockWsInstance.readyState = WebSocket.OPEN;
      newMockWsInstance.send = jest.fn();
      newMockWsInstance.close = jest.fn();
      MockWebSocket.mockImplementation(() => newMockWsInstance as unknown as WebSocket);

      // Simulate successful reconnection
      newMockWsInstance.emit('open');

      expect(reconnectedHandler).toHaveBeenCalled();
    });

    it('should resubscribe after reconnection', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Subscribe while connected
      ws.subscribeToTask('task-123');
      const initialSendCalls = (mockWsInstance.send as jest.Mock).mock.calls.length;

      // Simulate unexpected disconnect
      mockWsInstance.emit('close', { code: 1006, reason: 'Connection lost' });

      // Advance timer to trigger reconnect
      jest.advanceTimersByTime(200);

      // Create new mock for reconnection
      MockWebSocket.mockClear();
      const newMockWsInstance = new EventEmitter() as jest.Mocked<WebSocket> & EventEmitter;
      newMockWsInstance.readyState = WebSocket.OPEN;
      newMockWsInstance.send = jest.fn();
      newMockWsInstance.close = jest.fn();
      MockWebSocket.mockImplementation(() => newMockWsInstance as unknown as WebSocket);

      // Simulate successful reconnection
      newMockWsInstance.emit('open');

      // Should have resent subscription
      expect(newMockWsInstance.send).toHaveBeenCalledWith(
        expect.stringContaining('"taskId":"task-123"')
      );
    });
  });

  describe('heartbeat', () => {
    it('should send ping at heartbeat interval', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      jest.advanceTimersByTime(1000);

      expect(mockWsInstance.send).toHaveBeenCalledWith(
        JSON.stringify({ type: 'ping' })
      );
    });

    it('should close connection on heartbeat timeout', async () => {
      const errorHandler = jest.fn();
      ws.on('error', errorHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Trigger heartbeat
      jest.advanceTimersByTime(1000);

      // Heartbeat timeout
      jest.advanceTimersByTime(500);

      expect(errorHandler).toHaveBeenCalledWith(
        expect.objectContaining({
          message: 'Heartbeat timeout - server not responding',
        })
      );
      expect(mockWsInstance.close).toHaveBeenCalledWith(4000, 'Heartbeat timeout');
    });

    it('should reset heartbeat timeout on message', async () => {
      const errorHandler = jest.fn();
      ws.on('error', errorHandler);

      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      // Trigger heartbeat
      jest.advanceTimersByTime(1000);

      // Receive message before timeout
      jest.advanceTimersByTime(400);
      const message = JSON.stringify({
        type: WebSocketEventType.HEARTBEAT,
        payload: { serverTime: '2024-01-01T00:00:00Z', connectionId: 'conn-1' },
        timestamp: '2024-01-01T00:00:00Z',
      });
      mockWsInstance.emit('message', message);

      // Wait past original timeout
      jest.advanceTimersByTime(200);

      expect(errorHandler).not.toHaveBeenCalled();
    });

    it('should stop heartbeat on disconnect', async () => {
      const connectPromise = ws.connect();
      mockWsInstance.readyState = WebSocket.OPEN;
      mockWsInstance.emit('open');
      await connectPromise;

      const sendCallsBefore = (mockWsInstance.send as jest.Mock).mock.calls.length;

      ws.disconnect();
      mockWsInstance.emit('close', { code: 1000, reason: 'Client disconnecting' });

      jest.advanceTimersByTime(2000);

      // No additional pings should be sent
      expect((mockWsInstance.send as jest.Mock).mock.calls.length).toBe(sendCallsBefore);
    });
  });
});
