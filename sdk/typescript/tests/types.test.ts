/**
 * Tests for types and type guards
 */

import {
  TaskStatus,
  TaskPriority,
  AgentStatus,
  ApprovalStatus,
  DAGStatus,
  WebSocketEventType,
  Task,
  Agent,
  DAG,
  DAGNode,
  DAGEdge,
  Approval,
  DAGExecution,
  TaskLog,
  PaginatedResponse,
  ApiResponse,
  HealthCheckResponse,
  WebSocketMessage,
  WebSocketSubscription,
} from '../src';

describe('Enums', () => {
  describe('TaskStatus', () => {
    it('should have all expected values', () => {
      expect(TaskStatus.PENDING).toBe('pending');
      expect(TaskStatus.QUEUED).toBe('queued');
      expect(TaskStatus.RUNNING).toBe('running');
      expect(TaskStatus.PAUSED).toBe('paused');
      expect(TaskStatus.COMPLETED).toBe('completed');
      expect(TaskStatus.FAILED).toBe('failed');
      expect(TaskStatus.CANCELLED).toBe('cancelled');
      expect(TaskStatus.WAITING_APPROVAL).toBe('waiting_approval');
    });

    it('should have 8 status values', () => {
      const values = Object.values(TaskStatus);
      expect(values).toHaveLength(8);
    });
  });

  describe('TaskPriority', () => {
    it('should have all expected values', () => {
      expect(TaskPriority.LOW).toBe('low');
      expect(TaskPriority.NORMAL).toBe('normal');
      expect(TaskPriority.HIGH).toBe('high');
      expect(TaskPriority.CRITICAL).toBe('critical');
    });

    it('should have 4 priority values', () => {
      const values = Object.values(TaskPriority);
      expect(values).toHaveLength(4);
    });
  });

  describe('AgentStatus', () => {
    it('should have all expected values', () => {
      expect(AgentStatus.IDLE).toBe('idle');
      expect(AgentStatus.BUSY).toBe('busy');
      expect(AgentStatus.OFFLINE).toBe('offline');
      expect(AgentStatus.ERROR).toBe('error');
    });

    it('should have 4 status values', () => {
      const values = Object.values(AgentStatus);
      expect(values).toHaveLength(4);
    });
  });

  describe('ApprovalStatus', () => {
    it('should have all expected values', () => {
      expect(ApprovalStatus.PENDING).toBe('pending');
      expect(ApprovalStatus.APPROVED).toBe('approved');
      expect(ApprovalStatus.REJECTED).toBe('rejected');
      expect(ApprovalStatus.EXPIRED).toBe('expired');
    });

    it('should have 4 status values', () => {
      const values = Object.values(ApprovalStatus);
      expect(values).toHaveLength(4);
    });
  });

  describe('DAGStatus', () => {
    it('should have all expected values', () => {
      expect(DAGStatus.DRAFT).toBe('draft');
      expect(DAGStatus.ACTIVE).toBe('active');
      expect(DAGStatus.RUNNING).toBe('running');
      expect(DAGStatus.COMPLETED).toBe('completed');
      expect(DAGStatus.FAILED).toBe('failed');
      expect(DAGStatus.PAUSED).toBe('paused');
    });

    it('should have 6 status values', () => {
      const values = Object.values(DAGStatus);
      expect(values).toHaveLength(6);
    });
  });

  describe('WebSocketEventType', () => {
    it('should have all expected values', () => {
      expect(WebSocketEventType.TASK_CREATED).toBe('task.created');
      expect(WebSocketEventType.TASK_UPDATED).toBe('task.updated');
      expect(WebSocketEventType.TASK_COMPLETED).toBe('task.completed');
      expect(WebSocketEventType.TASK_FAILED).toBe('task.failed');
      expect(WebSocketEventType.AGENT_STATUS_CHANGED).toBe('agent.status_changed');
      expect(WebSocketEventType.DAG_STARTED).toBe('dag.started');
      expect(WebSocketEventType.DAG_COMPLETED).toBe('dag.completed');
      expect(WebSocketEventType.DAG_FAILED).toBe('dag.failed');
      expect(WebSocketEventType.APPROVAL_REQUIRED).toBe('approval.required');
      expect(WebSocketEventType.APPROVAL_RESOLVED).toBe('approval.resolved');
      expect(WebSocketEventType.LOG_MESSAGE).toBe('log.message');
      expect(WebSocketEventType.HEARTBEAT).toBe('heartbeat');
    });

    it('should have 12 event types', () => {
      const values = Object.values(WebSocketEventType);
      expect(values).toHaveLength(12);
    });
  });
});

describe('Type Validation', () => {
  describe('Task', () => {
    it('should accept valid task object', () => {
      const task: Task = {
        id: 'task-123',
        name: 'Test Task',
        status: TaskStatus.PENDING,
        priority: TaskPriority.NORMAL,
        retryCount: 0,
        maxRetries: 3,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(task.id).toBe('task-123');
      expect(task.status).toBe(TaskStatus.PENDING);
    });

    it('should accept task with optional fields', () => {
      const task: Task = {
        id: 'task-123',
        name: 'Test Task',
        description: 'A test task',
        status: TaskStatus.RUNNING,
        priority: TaskPriority.HIGH,
        agentId: 'agent-1',
        dagId: 'dag-1',
        dagNodeId: 'node-1',
        input: { key: 'value' },
        output: { result: 'success' },
        error: {
          code: 'ERR_001',
          message: 'Error occurred',
          details: { step: 3 },
          stack: 'Error stack trace',
        },
        metadata: { tag: 'test' },
        startedAt: '2024-01-01T00:00:00Z',
        completedAt: '2024-01-01T01:00:00Z',
        timeoutSeconds: 3600,
        retryCount: 1,
        maxRetries: 3,
        parentTaskId: 'parent-task-1',
        childTaskIds: ['child-1', 'child-2'],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T01:00:00Z',
      };

      expect(task.description).toBe('A test task');
      expect(task.agentId).toBe('agent-1');
      expect(task.childTaskIds).toHaveLength(2);
    });
  });

  describe('Agent', () => {
    it('should accept valid agent object', () => {
      const agent: Agent = {
        id: 'agent-123',
        name: 'Test Agent',
        status: AgentStatus.IDLE,
        capabilities: ['task-execution'],
        stats: {
          totalTasksCompleted: 100,
          totalTasksFailed: 5,
          averageTaskDuration: 5000,
          uptime: 86400,
        },
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(agent.id).toBe('agent-123');
      expect(agent.status).toBe(AgentStatus.IDLE);
      expect(agent.stats.totalTasksCompleted).toBe(100);
    });

    it('should accept agent with optional fields', () => {
      const agent: Agent = {
        id: 'agent-123',
        name: 'Test Agent',
        description: 'A test agent',
        status: AgentStatus.BUSY,
        capabilities: ['task-execution', 'data-processing'],
        currentTaskId: 'task-1',
        lastHeartbeat: '2024-01-01T00:00:00Z',
        metadata: { version: '1.0.0' },
        stats: {
          totalTasksCompleted: 100,
          totalTasksFailed: 5,
          averageTaskDuration: 5000,
          uptime: 86400,
        },
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(agent.currentTaskId).toBe('task-1');
      expect(agent.metadata?.['version']).toBe('1.0.0');
    });
  });

  describe('DAG', () => {
    it('should accept valid DAG object', () => {
      const dag: DAG = {
        id: 'dag-123',
        name: 'Test DAG',
        status: DAGStatus.ACTIVE,
        nodes: [],
        edges: [],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(dag.id).toBe('dag-123');
      expect(dag.status).toBe(DAGStatus.ACTIVE);
    });

    it('should accept DAG with nodes and edges', () => {
      const nodes: DAGNode[] = [
        {
          id: 'node-1',
          name: 'Task Node',
          type: 'task',
          config: {
            taskTemplate: { name: 'Task 1' },
          },
        },
        {
          id: 'node-2',
          name: 'Condition Node',
          type: 'condition',
          config: {
            condition: {
              expression: 'result > 0',
              trueBranch: 'node-3',
              falseBranch: 'node-4',
            },
          },
        },
        {
          id: 'node-3',
          name: 'Approval Node',
          type: 'approval',
          config: {
            approvalConfig: {
              approvers: ['user-1'],
              requiredApprovals: 1,
              timeoutSeconds: 3600,
              autoApprove: false,
            },
          },
        },
        {
          id: 'node-4',
          name: 'Parallel Node',
          type: 'parallel',
          config: {
            parallelNodes: ['node-5', 'node-6'],
          },
        },
      ];

      const edges: DAGEdge[] = [
        {
          id: 'edge-1',
          sourceNodeId: 'node-1',
          targetNodeId: 'node-2',
        },
        {
          id: 'edge-2',
          sourceNodeId: 'node-2',
          targetNodeId: 'node-3',
          condition: 'true',
        },
      ];

      const dag: DAG = {
        id: 'dag-123',
        name: 'Complex DAG',
        description: 'A complex DAG',
        status: DAGStatus.RUNNING,
        nodes,
        edges,
        metadata: { version: '1' },
        startedAt: '2024-01-01T00:00:00Z',
        currentNodeIds: ['node-2'],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(dag.nodes).toHaveLength(4);
      expect(dag.edges).toHaveLength(2);
      expect(dag.currentNodeIds).toHaveLength(1);
    });
  });

  describe('Approval', () => {
    it('should accept valid approval object', () => {
      const approval: Approval = {
        id: 'approval-123',
        status: ApprovalStatus.PENDING,
        requestedBy: 'user-1',
        approvers: ['user-2', 'user-3'],
        requiredApprovals: 1,
        currentApprovals: [],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(approval.id).toBe('approval-123');
      expect(approval.status).toBe(ApprovalStatus.PENDING);
    });

    it('should accept approval with decisions', () => {
      const approval: Approval = {
        id: 'approval-123',
        taskId: 'task-1',
        dagId: 'dag-1',
        dagNodeId: 'node-1',
        status: ApprovalStatus.APPROVED,
        requestedBy: 'user-1',
        approvers: ['user-2', 'user-3'],
        requiredApprovals: 1,
        currentApprovals: [
          {
            approverId: 'user-2',
            decision: 'approved',
            comment: 'Looks good',
            timestamp: '2024-01-01T01:00:00Z',
          },
        ],
        reason: 'Please review',
        expiresAt: '2024-01-02T00:00:00Z',
        metadata: { priority: 'high' },
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T01:00:00Z',
      };

      expect(approval.currentApprovals).toHaveLength(1);
      expect(approval.currentApprovals[0].decision).toBe('approved');
    });
  });

  describe('DAGExecution', () => {
    it('should accept valid execution object', () => {
      const execution: DAGExecution = {
        id: 'exec-123',
        dagId: 'dag-123',
        status: DAGStatus.RUNNING,
        nodeExecutions: [],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(execution.id).toBe('exec-123');
      expect(execution.status).toBe(DAGStatus.RUNNING);
    });

    it('should accept execution with node executions', () => {
      const execution: DAGExecution = {
        id: 'exec-123',
        dagId: 'dag-123',
        status: DAGStatus.COMPLETED,
        input: { key: 'value' },
        output: { result: 'success' },
        nodeExecutions: [
          {
            nodeId: 'node-1',
            taskId: 'task-1',
            status: TaskStatus.COMPLETED,
            startedAt: '2024-01-01T00:00:00Z',
            completedAt: '2024-01-01T00:30:00Z',
            input: { nodeInput: 1 },
            output: { nodeOutput: 2 },
          },
          {
            nodeId: 'node-2',
            status: TaskStatus.FAILED,
            error: {
              code: 'NODE_ERROR',
              message: 'Node execution failed',
            },
          },
        ],
        startedAt: '2024-01-01T00:00:00Z',
        completedAt: '2024-01-01T01:00:00Z',
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T01:00:00Z',
      };

      expect(execution.nodeExecutions).toHaveLength(2);
      expect(execution.nodeExecutions[0].status).toBe(TaskStatus.COMPLETED);
    });
  });

  describe('TaskLog', () => {
    it('should accept valid task log', () => {
      const log: TaskLog = {
        id: 'log-123',
        taskId: 'task-123',
        level: 'info',
        message: 'Task started',
        timestamp: '2024-01-01T00:00:00Z',
      };

      expect(log.id).toBe('log-123');
      expect(log.level).toBe('info');
    });

    it('should accept all log levels', () => {
      const levels: Array<TaskLog['level']> = ['debug', 'info', 'warn', 'error'];

      levels.forEach((level) => {
        const log: TaskLog = {
          id: 'log-123',
          taskId: 'task-123',
          level,
          message: 'Test message',
          timestamp: '2024-01-01T00:00:00Z',
        };

        expect(log.level).toBe(level);
      });
    });

    it('should accept log with metadata', () => {
      const log: TaskLog = {
        id: 'log-123',
        taskId: 'task-123',
        level: 'info',
        message: 'Processing step 3',
        timestamp: '2024-01-01T00:00:00Z',
        metadata: {
          step: 3,
          duration: 1000,
        },
      };

      expect(log.metadata?.['step']).toBe(3);
    });
  });
});

describe('Response Types', () => {
  describe('PaginatedResponse', () => {
    it('should accept valid paginated response', () => {
      const response: PaginatedResponse<Task> = {
        data: [
          {
            id: 'task-1',
            name: 'Task 1',
            status: TaskStatus.PENDING,
            priority: TaskPriority.NORMAL,
            retryCount: 0,
            maxRetries: 3,
            createdAt: '2024-01-01T00:00:00Z',
            updatedAt: '2024-01-01T00:00:00Z',
          },
        ],
        pagination: {
          page: 1,
          limit: 20,
          total: 100,
          totalPages: 5,
          hasMore: true,
        },
      };

      expect(response.data).toHaveLength(1);
      expect(response.pagination.hasMore).toBe(true);
    });
  });

  describe('ApiResponse', () => {
    it('should accept valid API response', () => {
      const response: ApiResponse<Task> = {
        success: true,
        data: {
          id: 'task-1',
          name: 'Task 1',
          status: TaskStatus.PENDING,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        },
        message: 'Task created successfully',
      };

      expect(response.success).toBe(true);
      expect(response.data.id).toBe('task-1');
    });
  });

  describe('HealthCheckResponse', () => {
    it('should accept valid health check response', () => {
      const response: HealthCheckResponse = {
        status: 'healthy',
        version: '1.0.0',
        uptime: 86400,
        services: {
          database: 'up',
          queue: 'up',
          websocket: 'up',
        },
      };

      expect(response.status).toBe('healthy');
      expect(response.services.database).toBe('up');
    });

    it('should accept degraded status', () => {
      const response: HealthCheckResponse = {
        status: 'degraded',
        version: '1.0.0',
        uptime: 86400,
        services: {
          database: 'up',
          queue: 'down',
          websocket: 'up',
        },
      };

      expect(response.status).toBe('degraded');
      expect(response.services.queue).toBe('down');
    });

    it('should accept unhealthy status', () => {
      const response: HealthCheckResponse = {
        status: 'unhealthy',
        version: '1.0.0',
        uptime: 0,
        services: {
          database: 'down',
          queue: 'down',
          websocket: 'down',
        },
      };

      expect(response.status).toBe('unhealthy');
    });
  });
});

describe('WebSocket Types', () => {
  describe('WebSocketMessage', () => {
    it('should accept valid message', () => {
      const message: WebSocketMessage<{ task: Task }> = {
        type: WebSocketEventType.TASK_CREATED,
        payload: {
          task: {
            id: 'task-1',
            name: 'Task 1',
            status: TaskStatus.PENDING,
            priority: TaskPriority.NORMAL,
            retryCount: 0,
            maxRetries: 3,
            createdAt: '2024-01-01T00:00:00Z',
            updatedAt: '2024-01-01T00:00:00Z',
          },
        },
        timestamp: '2024-01-01T00:00:00Z',
      };

      expect(message.type).toBe(WebSocketEventType.TASK_CREATED);
      expect(message.payload.task.id).toBe('task-1');
    });

    it('should accept message with correlation ID', () => {
      const message: WebSocketMessage = {
        type: WebSocketEventType.HEARTBEAT,
        payload: {
          serverTime: '2024-01-01T00:00:00Z',
          connectionId: 'conn-123',
        },
        timestamp: '2024-01-01T00:00:00Z',
        correlationId: 'corr-123',
      };

      expect(message.correlationId).toBe('corr-123');
    });
  });

  describe('WebSocketSubscription', () => {
    it('should accept single event subscription', () => {
      const subscription: WebSocketSubscription = {
        event: WebSocketEventType.TASK_CREATED,
      };

      expect(subscription.event).toBe(WebSocketEventType.TASK_CREATED);
    });

    it('should accept multiple event subscription', () => {
      const subscription: WebSocketSubscription = {
        event: [
          WebSocketEventType.TASK_CREATED,
          WebSocketEventType.TASK_COMPLETED,
          WebSocketEventType.TASK_FAILED,
        ],
      };

      expect(subscription.event).toHaveLength(3);
    });

    it('should accept subscription with filters', () => {
      const subscription: WebSocketSubscription = {
        event: WebSocketEventType.TASK_UPDATED,
        filter: {
          taskId: 'task-123',
        },
      };

      expect(subscription.filter?.taskId).toBe('task-123');
    });

    it('should accept subscription with all filter types', () => {
      const subscription: WebSocketSubscription = {
        event: [
          WebSocketEventType.TASK_UPDATED,
          WebSocketEventType.AGENT_STATUS_CHANGED,
          WebSocketEventType.DAG_STARTED,
        ],
        filter: {
          taskId: 'task-123',
          agentId: 'agent-123',
          dagId: 'dag-123',
        },
      };

      expect(subscription.filter?.taskId).toBe('task-123');
      expect(subscription.filter?.agentId).toBe('agent-123');
      expect(subscription.filter?.dagId).toBe('dag-123');
    });
  });
});

describe('Type Guards', () => {
  // Type guard helper functions for runtime validation
  const isTask = (obj: unknown): obj is Task => {
    if (!obj || typeof obj !== 'object') return false;
    const task = obj as Partial<Task>;
    return (
      typeof task.id === 'string' &&
      typeof task.name === 'string' &&
      typeof task.status === 'string' &&
      Object.values(TaskStatus).includes(task.status as TaskStatus) &&
      typeof task.priority === 'string' &&
      Object.values(TaskPriority).includes(task.priority as TaskPriority) &&
      typeof task.retryCount === 'number' &&
      typeof task.maxRetries === 'number' &&
      typeof task.createdAt === 'string' &&
      typeof task.updatedAt === 'string'
    );
  };

  const isAgent = (obj: unknown): obj is Agent => {
    if (!obj || typeof obj !== 'object') return false;
    const agent = obj as Partial<Agent>;
    return (
      typeof agent.id === 'string' &&
      typeof agent.name === 'string' &&
      typeof agent.status === 'string' &&
      Object.values(AgentStatus).includes(agent.status as AgentStatus) &&
      Array.isArray(agent.capabilities) &&
      typeof agent.stats === 'object'
    );
  };

  const isTaskStatus = (value: unknown): value is TaskStatus => {
    return typeof value === 'string' && Object.values(TaskStatus).includes(value as TaskStatus);
  };

  const isWebSocketEventType = (value: unknown): value is WebSocketEventType => {
    return typeof value === 'string' && Object.values(WebSocketEventType).includes(value as WebSocketEventType);
  };

  describe('isTask', () => {
    it('should return true for valid task', () => {
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

      expect(isTask(task)).toBe(true);
    });

    it('should return false for invalid task', () => {
      expect(isTask(null)).toBe(false);
      expect(isTask(undefined)).toBe(false);
      expect(isTask({})).toBe(false);
      expect(isTask({ id: 'task-123' })).toBe(false);
      expect(isTask({ id: 123, name: 'Test' })).toBe(false);
    });

    it('should return false for invalid status', () => {
      const task = {
        id: 'task-123',
        name: 'Test Task',
        status: 'invalid_status',
        priority: TaskPriority.NORMAL,
        retryCount: 0,
        maxRetries: 3,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(isTask(task)).toBe(false);
    });
  });

  describe('isAgent', () => {
    it('should return true for valid agent', () => {
      const agent = {
        id: 'agent-123',
        name: 'Test Agent',
        status: AgentStatus.IDLE,
        capabilities: ['task-execution'],
        stats: {
          totalTasksCompleted: 0,
          totalTasksFailed: 0,
          averageTaskDuration: 0,
          uptime: 0,
        },
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      };

      expect(isAgent(agent)).toBe(true);
    });

    it('should return false for invalid agent', () => {
      expect(isAgent(null)).toBe(false);
      expect(isAgent({})).toBe(false);
      expect(isAgent({ id: 'agent-123' })).toBe(false);
    });
  });

  describe('isTaskStatus', () => {
    it('should return true for valid status', () => {
      Object.values(TaskStatus).forEach((status) => {
        expect(isTaskStatus(status)).toBe(true);
      });
    });

    it('should return false for invalid status', () => {
      expect(isTaskStatus('invalid')).toBe(false);
      expect(isTaskStatus(123)).toBe(false);
      expect(isTaskStatus(null)).toBe(false);
    });
  });

  describe('isWebSocketEventType', () => {
    it('should return true for valid event type', () => {
      Object.values(WebSocketEventType).forEach((eventType) => {
        expect(isWebSocketEventType(eventType)).toBe(true);
      });
    });

    it('should return false for invalid event type', () => {
      expect(isWebSocketEventType('invalid.event')).toBe(false);
      expect(isWebSocketEventType(123)).toBe(false);
      expect(isWebSocketEventType(null)).toBe(false);
    });
  });
});
