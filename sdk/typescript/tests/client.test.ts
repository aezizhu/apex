/**
 * Tests for ApexClient
 */

import axios from 'axios';
import MockAdapter from 'axios-mock-adapter';
import {
  ApexClient,
  createApexClient,
  TaskStatus,
  TaskPriority,
  AgentStatus,
  DAGStatus,
  ApprovalStatus,
  ApiRequestError,
  AuthenticationError,
  AuthorizationError,
  NotFoundError,
  ValidationError,
  RateLimitError,
  TimeoutError,
  MaxRetriesExceededError,
  Task,
  Agent,
  DAG,
  Approval,
  DAGExecution,
} from '../src';

// Mock axios for HTTP tests
jest.mock('axios');
const mockedAxios = axios as jest.Mocked<typeof axios>;

describe('ApexClient', () => {
  let client: ApexClient;
  let mockAxiosInstance: MockAdapter;
  let axiosInstance: ReturnType<typeof axios.create>;

  const baseConfig = {
    baseUrl: 'https://api.apex.test',
    apiKey: 'test-api-key',
    timeout: 5000,
    retries: 3,
    retryDelay: 100,
    maxRetryDelay: 1000,
  };

  beforeEach(() => {
    axiosInstance = axios.create();
    mockAxiosInstance = new MockAdapter(axiosInstance);

    mockedAxios.create.mockReturnValue(axiosInstance);
    client = new ApexClient(baseConfig);
  });

  afterEach(() => {
    mockAxiosInstance.reset();
    jest.clearAllMocks();
  });

  describe('constructor', () => {
    it('should create client with default configuration', () => {
      const minimalClient = new ApexClient({
        baseUrl: 'https://api.apex.test',
      });
      expect(minimalClient).toBeInstanceOf(ApexClient);
    });

    it('should strip trailing slash from baseUrl', () => {
      const clientWithSlash = new ApexClient({
        baseUrl: 'https://api.apex.test/',
      });
      expect(clientWithSlash).toBeInstanceOf(ApexClient);
    });

    it('should set custom headers', () => {
      const clientWithHeaders = new ApexClient({
        baseUrl: 'https://api.apex.test',
        headers: { 'X-Custom-Header': 'value' },
      });
      expect(clientWithHeaders).toBeInstanceOf(ApexClient);
    });
  });

  describe('createApexClient factory', () => {
    it('should create a new ApexClient instance', () => {
      const factoryClient = createApexClient(baseConfig);
      expect(factoryClient).toBeInstanceOf(ApexClient);
    });
  });

  describe('healthCheck', () => {
    it('should return health status', async () => {
      const healthResponse = {
        status: 'healthy',
        version: '1.0.0',
        uptime: 3600,
        services: {
          database: 'up',
          queue: 'up',
          websocket: 'up',
        },
      };

      mockAxiosInstance.onGet('/health').reply(200, {
        success: true,
        data: healthResponse,
      });

      const result = await client.healthCheck();
      expect(result).toEqual(healthResponse);
    });
  });

  describe('Task API', () => {
    const mockTask: Task = {
      id: 'task-123',
      name: 'Test Task',
      description: 'A test task',
      status: TaskStatus.PENDING,
      priority: TaskPriority.NORMAL,
      retryCount: 0,
      maxRetries: 3,
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    describe('listTasks', () => {
      it('should list tasks without filters', async () => {
        const paginatedResponse = {
          data: [mockTask],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/tasks').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listTasks();
        expect(result).toEqual(paginatedResponse);
      });

      it('should list tasks with filters', async () => {
        const paginatedResponse = {
          data: [mockTask],
          pagination: {
            page: 1,
            limit: 10,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet(/\/api\/v1\/tasks\?/).reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listTasks({
          status: TaskStatus.PENDING,
          priority: TaskPriority.HIGH,
          limit: 10,
        });
        expect(result).toEqual(paginatedResponse);
      });

      it('should handle array filters', async () => {
        const paginatedResponse = {
          data: [],
          pagination: {
            page: 1,
            limit: 20,
            total: 0,
            totalPages: 0,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet(/\/api\/v1\/tasks\?/).reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listTasks({
          status: [TaskStatus.PENDING, TaskStatus.RUNNING],
        });
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('getTask', () => {
      it('should get a task by ID', async () => {
        mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
          success: true,
          data: mockTask,
        });

        const result = await client.getTask('task-123');
        expect(result).toEqual(mockTask);
      });

      it('should throw NotFoundError for non-existent task', async () => {
        mockAxiosInstance.onGet('/api/v1/tasks/non-existent').reply(404, {
          success: false,
          error: {
            code: 'NOT_FOUND',
            message: 'Task not found',
            resourceType: 'Task',
            resourceId: 'non-existent',
          },
        });

        await expect(client.getTask('non-existent')).rejects.toThrow(NotFoundError);
      });
    });

    describe('createTask', () => {
      it('should create a new task', async () => {
        const createRequest = {
          name: 'New Task',
          description: 'A new task',
          priority: TaskPriority.HIGH,
        };

        const createdTask = {
          ...mockTask,
          ...createRequest,
          id: 'task-456',
        };

        mockAxiosInstance.onPost('/api/v1/tasks').reply(201, {
          success: true,
          data: createdTask,
        });

        const result = await client.createTask(createRequest);
        expect(result.name).toBe('New Task');
      });

      it('should throw ValidationError for invalid task data', async () => {
        mockAxiosInstance.onPost('/api/v1/tasks').reply(400, {
          success: false,
          error: {
            code: 'VALIDATION_ERROR',
            message: 'Validation failed',
            validationErrors: [
              { field: 'name', message: 'Name is required' },
            ],
          },
        });

        await expect(client.createTask({ name: '' })).rejects.toThrow(ValidationError);
      });
    });

    describe('updateTask', () => {
      it('should update an existing task', async () => {
        const updatedTask = {
          ...mockTask,
          name: 'Updated Task',
        };

        mockAxiosInstance.onPatch('/api/v1/tasks/task-123').reply(200, {
          success: true,
          data: updatedTask,
        });

        const result = await client.updateTask('task-123', { name: 'Updated Task' });
        expect(result.name).toBe('Updated Task');
      });
    });

    describe('deleteTask', () => {
      it('should delete a task', async () => {
        mockAxiosInstance.onDelete('/api/v1/tasks/task-123').reply(204, {
          success: true,
          data: null,
        });

        await expect(client.deleteTask('task-123')).resolves.not.toThrow();
      });
    });

    describe('cancelTask', () => {
      it('should cancel a running task', async () => {
        const cancelledTask = {
          ...mockTask,
          status: TaskStatus.CANCELLED,
        };

        mockAxiosInstance.onPost('/api/v1/tasks/task-123/cancel').reply(200, {
          success: true,
          data: cancelledTask,
        });

        const result = await client.cancelTask('task-123');
        expect(result.status).toBe(TaskStatus.CANCELLED);
      });
    });

    describe('retryTask', () => {
      it('should retry a failed task', async () => {
        const retriedTask = {
          ...mockTask,
          status: TaskStatus.QUEUED,
          retryCount: 1,
        };

        mockAxiosInstance.onPost('/api/v1/tasks/task-123/retry').reply(200, {
          success: true,
          data: retriedTask,
        });

        const result = await client.retryTask('task-123');
        expect(result.status).toBe(TaskStatus.QUEUED);
        expect(result.retryCount).toBe(1);
      });
    });

    describe('pauseTask', () => {
      it('should pause a running task', async () => {
        const pausedTask = {
          ...mockTask,
          status: TaskStatus.PAUSED,
        };

        mockAxiosInstance.onPost('/api/v1/tasks/task-123/pause').reply(200, {
          success: true,
          data: pausedTask,
        });

        const result = await client.pauseTask('task-123');
        expect(result.status).toBe(TaskStatus.PAUSED);
      });
    });

    describe('resumeTask', () => {
      it('should resume a paused task', async () => {
        const resumedTask = {
          ...mockTask,
          status: TaskStatus.RUNNING,
        };

        mockAxiosInstance.onPost('/api/v1/tasks/task-123/resume').reply(200, {
          success: true,
          data: resumedTask,
        });

        const result = await client.resumeTask('task-123');
        expect(result.status).toBe(TaskStatus.RUNNING);
      });
    });

    describe('getTaskLogs', () => {
      it('should get task logs', async () => {
        const logs = [
          {
            id: 'log-1',
            taskId: 'task-123',
            level: 'info',
            message: 'Task started',
            timestamp: '2024-01-01T00:00:00Z',
          },
        ];

        mockAxiosInstance.onGet('/api/v1/tasks/task-123/logs').reply(200, {
          success: true,
          data: logs,
        });

        const result = await client.getTaskLogs('task-123');
        expect(result).toEqual(logs);
      });

      it('should get task logs with params', async () => {
        const logs: never[] = [];

        mockAxiosInstance.onGet(/\/api\/v1\/tasks\/task-123\/logs\?/).reply(200, {
          success: true,
          data: logs,
        });

        const result = await client.getTaskLogs('task-123', {
          limit: 50,
          level: 'error',
        });
        expect(result).toEqual(logs);
      });
    });

    describe('getChildTasks', () => {
      it('should get child tasks', async () => {
        const paginatedResponse = {
          data: [],
          pagination: {
            page: 1,
            limit: 20,
            total: 0,
            totalPages: 0,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/tasks/task-123/children').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.getChildTasks('task-123');
        expect(result).toEqual(paginatedResponse);
      });
    });
  });

  describe('Agent API', () => {
    const mockAgent: Agent = {
      id: 'agent-123',
      name: 'Test Agent',
      description: 'A test agent',
      status: AgentStatus.IDLE,
      capabilities: ['task-execution'],
      stats: {
        totalTasksCompleted: 10,
        totalTasksFailed: 2,
        averageTaskDuration: 5000,
        uptime: 86400,
      },
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    describe('listAgents', () => {
      it('should list agents', async () => {
        const paginatedResponse = {
          data: [mockAgent],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/agents').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listAgents();
        expect(result).toEqual(paginatedResponse);
      });

      it('should list agents with filters', async () => {
        const paginatedResponse = {
          data: [mockAgent],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet(/\/api\/v1\/agents\?/).reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listAgents({
          status: AgentStatus.IDLE,
          capabilities: ['task-execution'],
        });
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('getAgent', () => {
      it('should get an agent by ID', async () => {
        mockAxiosInstance.onGet('/api/v1/agents/agent-123').reply(200, {
          success: true,
          data: mockAgent,
        });

        const result = await client.getAgent('agent-123');
        expect(result).toEqual(mockAgent);
      });
    });

    describe('createAgent', () => {
      it('should create a new agent', async () => {
        const createRequest = {
          name: 'New Agent',
          capabilities: ['task-execution', 'data-processing'],
        };

        mockAxiosInstance.onPost('/api/v1/agents').reply(201, {
          success: true,
          data: { ...mockAgent, ...createRequest, id: 'agent-456' },
        });

        const result = await client.createAgent(createRequest);
        expect(result.name).toBe('New Agent');
      });
    });

    describe('updateAgent', () => {
      it('should update an agent', async () => {
        const updatedAgent = { ...mockAgent, name: 'Updated Agent' };

        mockAxiosInstance.onPatch('/api/v1/agents/agent-123').reply(200, {
          success: true,
          data: updatedAgent,
        });

        const result = await client.updateAgent('agent-123', { name: 'Updated Agent' });
        expect(result.name).toBe('Updated Agent');
      });
    });

    describe('deleteAgent', () => {
      it('should delete an agent', async () => {
        mockAxiosInstance.onDelete('/api/v1/agents/agent-123').reply(204, {
          success: true,
          data: null,
        });

        await expect(client.deleteAgent('agent-123')).resolves.not.toThrow();
      });
    });

    describe('getAgentTasks', () => {
      it('should get tasks assigned to an agent', async () => {
        const paginatedResponse = {
          data: [],
          pagination: {
            page: 1,
            limit: 20,
            total: 0,
            totalPages: 0,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/agents/agent-123/tasks').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.getAgentTasks('agent-123');
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('assignTask', () => {
      it('should assign a task to an agent', async () => {
        const task: Task = {
          id: 'task-123',
          name: 'Test Task',
          status: TaskStatus.PENDING,
          priority: TaskPriority.NORMAL,
          agentId: 'agent-123',
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onPost('/api/v1/agents/agent-123/assign').reply(200, {
          success: true,
          data: task,
        });

        const result = await client.assignTask('agent-123', 'task-123');
        expect(result.agentId).toBe('agent-123');
      });
    });

    describe('unassignTask', () => {
      it('should unassign a task from an agent', async () => {
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

        mockAxiosInstance.onPost('/api/v1/agents/agent-123/unassign').reply(200, {
          success: true,
          data: task,
        });

        const result = await client.unassignTask('agent-123', 'task-123');
        expect(result.agentId).toBeUndefined();
      });
    });
  });

  describe('DAG API', () => {
    const mockDAG: DAG = {
      id: 'dag-123',
      name: 'Test DAG',
      description: 'A test DAG',
      status: DAGStatus.ACTIVE,
      nodes: [
        {
          id: 'node-1',
          name: 'First Node',
          type: 'task',
          config: {
            taskTemplate: { name: 'Task 1' },
          },
        },
      ],
      edges: [
        {
          id: 'edge-1',
          sourceNodeId: 'node-1',
          targetNodeId: 'node-2',
        },
      ],
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    const mockExecution: DAGExecution = {
      id: 'exec-123',
      dagId: 'dag-123',
      status: DAGStatus.RUNNING,
      nodeExecutions: [],
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    describe('listDAGs', () => {
      it('should list DAGs', async () => {
        const paginatedResponse = {
          data: [mockDAG],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/dags').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listDAGs();
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('getDAG', () => {
      it('should get a DAG by ID', async () => {
        mockAxiosInstance.onGet('/api/v1/dags/dag-123').reply(200, {
          success: true,
          data: mockDAG,
        });

        const result = await client.getDAG('dag-123');
        expect(result).toEqual(mockDAG);
      });
    });

    describe('createDAG', () => {
      it('should create a new DAG', async () => {
        const createRequest = {
          name: 'New DAG',
          nodes: [
            { id: 'node-1', name: 'Node 1', type: 'task' as const, config: {} },
          ],
          edges: [],
        };

        mockAxiosInstance.onPost('/api/v1/dags').reply(201, {
          success: true,
          data: { ...mockDAG, ...createRequest, id: 'dag-456' },
        });

        const result = await client.createDAG(createRequest);
        expect(result.name).toBe('New DAG');
      });
    });

    describe('updateDAG', () => {
      it('should update a DAG', async () => {
        const updatedDAG = { ...mockDAG, name: 'Updated DAG' };

        mockAxiosInstance.onPatch('/api/v1/dags/dag-123').reply(200, {
          success: true,
          data: updatedDAG,
        });

        const result = await client.updateDAG('dag-123', { name: 'Updated DAG' });
        expect(result.name).toBe('Updated DAG');
      });
    });

    describe('deleteDAG', () => {
      it('should delete a DAG', async () => {
        mockAxiosInstance.onDelete('/api/v1/dags/dag-123').reply(204, {
          success: true,
          data: null,
        });

        await expect(client.deleteDAG('dag-123')).resolves.not.toThrow();
      });
    });

    describe('startDAG', () => {
      it('should start a DAG execution', async () => {
        mockAxiosInstance.onPost('/api/v1/dags/dag-123/start').reply(200, {
          success: true,
          data: mockExecution,
        });

        const result = await client.startDAG('dag-123');
        expect(result).toEqual(mockExecution);
      });

      it('should start a DAG with input', async () => {
        mockAxiosInstance.onPost('/api/v1/dags/dag-123/start').reply(200, {
          success: true,
          data: { ...mockExecution, input: { key: 'value' } },
        });

        const result = await client.startDAG('dag-123', { key: 'value' });
        expect(result.input).toEqual({ key: 'value' });
      });
    });

    describe('stopDAG', () => {
      it('should stop a running DAG', async () => {
        const stoppedDAG = { ...mockDAG, status: DAGStatus.COMPLETED };

        mockAxiosInstance.onPost('/api/v1/dags/dag-123/stop').reply(200, {
          success: true,
          data: stoppedDAG,
        });

        const result = await client.stopDAG('dag-123');
        expect(result.status).toBe(DAGStatus.COMPLETED);
      });
    });

    describe('pauseDAG', () => {
      it('should pause a running DAG', async () => {
        const pausedDAG = { ...mockDAG, status: DAGStatus.PAUSED };

        mockAxiosInstance.onPost('/api/v1/dags/dag-123/pause').reply(200, {
          success: true,
          data: pausedDAG,
        });

        const result = await client.pauseDAG('dag-123');
        expect(result.status).toBe(DAGStatus.PAUSED);
      });
    });

    describe('resumeDAG', () => {
      it('should resume a paused DAG', async () => {
        const resumedDAG = { ...mockDAG, status: DAGStatus.RUNNING };

        mockAxiosInstance.onPost('/api/v1/dags/dag-123/resume').reply(200, {
          success: true,
          data: resumedDAG,
        });

        const result = await client.resumeDAG('dag-123');
        expect(result.status).toBe(DAGStatus.RUNNING);
      });
    });

    describe('getDAGExecutions', () => {
      it('should get DAG execution history', async () => {
        const paginatedResponse = {
          data: [mockExecution],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/dags/dag-123/executions').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.getDAGExecutions('dag-123');
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('getDAGExecution', () => {
      it('should get a specific DAG execution', async () => {
        mockAxiosInstance.onGet('/api/v1/dags/dag-123/executions/exec-123').reply(200, {
          success: true,
          data: mockExecution,
        });

        const result = await client.getDAGExecution('dag-123', 'exec-123');
        expect(result).toEqual(mockExecution);
      });
    });
  });

  describe('Approval API', () => {
    const mockApproval: Approval = {
      id: 'approval-123',
      status: ApprovalStatus.PENDING,
      requestedBy: 'user-1',
      approvers: ['user-2', 'user-3'],
      requiredApprovals: 1,
      currentApprovals: [],
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    describe('listApprovals', () => {
      it('should list approvals', async () => {
        const paginatedResponse = {
          data: [mockApproval],
          pagination: {
            page: 1,
            limit: 20,
            total: 1,
            totalPages: 1,
            hasMore: false,
          },
        };

        mockAxiosInstance.onGet('/api/v1/approvals').reply(200, {
          success: true,
          data: paginatedResponse,
        });

        const result = await client.listApprovals();
        expect(result).toEqual(paginatedResponse);
      });
    });

    describe('getApproval', () => {
      it('should get an approval by ID', async () => {
        mockAxiosInstance.onGet('/api/v1/approvals/approval-123').reply(200, {
          success: true,
          data: mockApproval,
        });

        const result = await client.getApproval('approval-123');
        expect(result).toEqual(mockApproval);
      });
    });

    describe('createApproval', () => {
      it('should create a new approval request', async () => {
        const createRequest = {
          taskId: 'task-123',
          approvers: ['user-2'],
          reason: 'Please review',
        };

        mockAxiosInstance.onPost('/api/v1/approvals').reply(201, {
          success: true,
          data: { ...mockApproval, ...createRequest, id: 'approval-456' },
        });

        const result = await client.createApproval(createRequest);
        expect(result.taskId).toBe('task-123');
      });
    });

    describe('respondToApproval', () => {
      it('should approve a request', async () => {
        const approvedApproval = {
          ...mockApproval,
          status: ApprovalStatus.APPROVED,
          currentApprovals: [
            {
              approverId: 'user-2',
              decision: 'approved',
              timestamp: '2024-01-01T01:00:00Z',
            },
          ],
        };

        mockAxiosInstance.onPost('/api/v1/approvals/approval-123/respond').reply(200, {
          success: true,
          data: approvedApproval,
        });

        const result = await client.respondToApproval('approval-123', {
          decision: 'approved',
          comment: 'Looks good',
        });
        expect(result.status).toBe(ApprovalStatus.APPROVED);
      });

      it('should reject a request', async () => {
        const rejectedApproval = {
          ...mockApproval,
          status: ApprovalStatus.REJECTED,
          currentApprovals: [
            {
              approverId: 'user-2',
              decision: 'rejected',
              comment: 'Needs changes',
              timestamp: '2024-01-01T01:00:00Z',
            },
          ],
        };

        mockAxiosInstance.onPost('/api/v1/approvals/approval-123/respond').reply(200, {
          success: true,
          data: rejectedApproval,
        });

        const result = await client.respondToApproval('approval-123', {
          decision: 'rejected',
          comment: 'Needs changes',
        });
        expect(result.status).toBe(ApprovalStatus.REJECTED);
      });
    });

    describe('cancelApproval', () => {
      it('should cancel an approval request', async () => {
        const cancelledApproval = {
          ...mockApproval,
          status: ApprovalStatus.EXPIRED,
        };

        mockAxiosInstance.onPost('/api/v1/approvals/approval-123/cancel').reply(200, {
          success: true,
          data: cancelledApproval,
        });

        const result = await client.cancelApproval('approval-123');
        expect(result.status).toBe(ApprovalStatus.EXPIRED);
      });
    });

    describe('getPendingApprovals', () => {
      it('should get pending approvals for an approver', async () => {
        mockAxiosInstance.onGet(/\/api\/v1\/approvals\/pending\?/).reply(200, {
          success: true,
          data: [mockApproval],
        });

        const result = await client.getPendingApprovals('user-2');
        expect(result).toEqual([mockApproval]);
      });
    });
  });

  describe('Retry Logic', () => {
    const mockTask: Task = {
      id: 'task-123',
      name: 'Test Task',
      status: TaskStatus.PENDING,
      priority: TaskPriority.NORMAL,
      retryCount: 0,
      maxRetries: 3,
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    it('should retry on 5xx errors', async () => {
      let attempts = 0;
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(() => {
        attempts++;
        if (attempts < 3) {
          return [500, { success: false, error: { code: 'SERVER_ERROR', message: 'Internal error' } }];
        }
        return [200, { success: true, data: mockTask }];
      });

      const result = await client.getTask('task-123');
      expect(result).toEqual(mockTask);
      expect(attempts).toBe(3);
    });

    it('should retry on rate limit (429)', async () => {
      let attempts = 0;
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(() => {
        attempts++;
        if (attempts < 2) {
          return [429, { success: false, error: { code: 'RATE_LIMIT_EXCEEDED', message: 'Too many requests' } }];
        }
        return [200, { success: true, data: mockTask }];
      });

      const result = await client.getTask('task-123');
      expect(result).toEqual(mockTask);
      expect(attempts).toBe(2);
    });

    it('should not retry on 4xx errors (except 429)', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(400, {
        success: false,
        error: { code: 'BAD_REQUEST', message: 'Bad request' },
      });

      await expect(client.getTask('task-123')).rejects.toThrow(ApiRequestError);
    });

    it('should throw MaxRetriesExceededError after max attempts', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(500, {
        success: false,
        error: { code: 'SERVER_ERROR', message: 'Internal error' },
      });

      await expect(client.getTask('task-123')).rejects.toThrow(MaxRetriesExceededError);
    });
  });

  describe('Error Handling', () => {
    it('should handle 401 authentication errors', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks').reply(401, {
        success: false,
        error: { code: 'AUTHENTICATION_ERROR', message: 'Invalid API key' },
      });

      await expect(client.listTasks()).rejects.toThrow(AuthenticationError);
    });

    it('should handle 403 authorization errors', async () => {
      mockAxiosInstance.onDelete('/api/v1/tasks/task-123').reply(403, {
        success: false,
        error: {
          code: 'AUTHORIZATION_ERROR',
          message: 'Access denied',
          resource: 'task',
          action: 'delete',
        },
      });

      await expect(client.deleteTask('task-123')).rejects.toThrow(AuthorizationError);
    });

    it('should handle 429 rate limit errors', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks').networkError();
      mockAxiosInstance.onGet('/api/v1/tasks').reply(429, {
        success: false,
        error: {
          code: 'RATE_LIMIT_EXCEEDED',
          message: 'Rate limit exceeded',
          retryAfter: 60,
          limit: 100,
          remaining: 0,
        },
      });

      await expect(client.listTasks()).rejects.toThrow(RateLimitError);
    });
  });

  describe('Request Configuration', () => {
    const mockTask: Task = {
      id: 'task-123',
      name: 'Test Task',
      status: TaskStatus.PENDING,
      priority: TaskPriority.NORMAL,
      retryCount: 0,
      maxRetries: 3,
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
    };

    it('should support custom timeout', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
        success: true,
        data: mockTask,
      });

      const result = await client.getTask('task-123', { timeout: 1000 });
      expect(result).toEqual(mockTask);
    });

    it('should support custom headers', async () => {
      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
        success: true,
        data: mockTask,
      });

      const result = await client.getTask('task-123', {
        headers: { 'X-Custom-Header': 'value' },
      });
      expect(result).toEqual(mockTask);
    });

    it('should support abort signal', async () => {
      const controller = new AbortController();

      mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
        success: true,
        data: mockTask,
      });

      const result = await client.getTask('task-123', { signal: controller.signal });
      expect(result).toEqual(mockTask);
    });
  });

  describe('Convenience Methods', () => {
    describe('waitForTask', () => {
      it('should wait for task completion with polling', async () => {
        const completedTask: Task = {
          id: 'task-123',
          name: 'Test Task',
          status: TaskStatus.COMPLETED,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        let pollCount = 0;
        mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(() => {
          pollCount++;
          const status = pollCount < 3 ? TaskStatus.RUNNING : TaskStatus.COMPLETED;
          return [200, { success: true, data: { ...completedTask, status } }];
        });

        const result = await client.waitForTask('task-123', { pollInterval: 50 });
        expect(result.status).toBe(TaskStatus.COMPLETED);
        expect(pollCount).toBeGreaterThanOrEqual(3);
      });

      it('should throw on task failure', async () => {
        const failedTask: Task = {
          id: 'task-123',
          name: 'Test Task',
          status: TaskStatus.FAILED,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          error: { code: 'EXECUTION_ERROR', message: 'Task execution failed' },
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
          success: true,
          data: failedTask,
        });

        await expect(
          client.waitForTask('task-123', { pollInterval: 50 })
        ).rejects.toThrow('Task task-123 failed');
      });

      it('should throw on task cancellation', async () => {
        const cancelledTask: Task = {
          id: 'task-123',
          name: 'Test Task',
          status: TaskStatus.CANCELLED,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
          success: true,
          data: cancelledTask,
        });

        await expect(
          client.waitForTask('task-123', { pollInterval: 50 })
        ).rejects.toThrow('Task task-123 cancelled');
      });

      it('should timeout after specified duration', async () => {
        const runningTask: Task = {
          id: 'task-123',
          name: 'Test Task',
          status: TaskStatus.RUNNING,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onGet('/api/v1/tasks/task-123').reply(200, {
          success: true,
          data: runningTask,
        });

        await expect(
          client.waitForTask('task-123', { pollInterval: 50, timeout: 200 })
        ).rejects.toThrow(TimeoutError);
      });
    });

    describe('runTask', () => {
      it('should create and wait for task completion', async () => {
        const pendingTask: Task = {
          id: 'task-456',
          name: 'New Task',
          status: TaskStatus.PENDING,
          priority: TaskPriority.NORMAL,
          retryCount: 0,
          maxRetries: 3,
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        const completedTask = { ...pendingTask, status: TaskStatus.COMPLETED };

        mockAxiosInstance.onPost('/api/v1/tasks').reply(201, {
          success: true,
          data: pendingTask,
        });

        let pollCount = 0;
        mockAxiosInstance.onGet('/api/v1/tasks/task-456').reply(() => {
          pollCount++;
          const status = pollCount < 2 ? TaskStatus.RUNNING : TaskStatus.COMPLETED;
          return [200, { success: true, data: { ...pendingTask, status } }];
        });

        const result = await client.runTask(
          { name: 'New Task' },
          { pollInterval: 50 }
        );
        expect(result.status).toBe(TaskStatus.COMPLETED);
      });
    });

    describe('waitForDAG', () => {
      it('should wait for DAG execution completion', async () => {
        const mockExecution: DAGExecution = {
          id: 'exec-123',
          dagId: 'dag-123',
          status: DAGStatus.COMPLETED,
          nodeExecutions: [],
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        let pollCount = 0;
        mockAxiosInstance.onGet('/api/v1/dags/dag-123/executions/exec-123').reply(() => {
          pollCount++;
          const status = pollCount < 2 ? DAGStatus.RUNNING : DAGStatus.COMPLETED;
          return [200, { success: true, data: { ...mockExecution, status } }];
        });

        const result = await client.waitForDAG('dag-123', 'exec-123', { pollInterval: 50 });
        expect(result.status).toBe(DAGStatus.COMPLETED);
      });

      it('should throw on DAG execution failure', async () => {
        const mockExecution: DAGExecution = {
          id: 'exec-123',
          dagId: 'dag-123',
          status: DAGStatus.FAILED,
          nodeExecutions: [],
          error: { code: 'NODE_FAILED', message: 'Node execution failed' },
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onGet('/api/v1/dags/dag-123/executions/exec-123').reply(200, {
          success: true,
          data: mockExecution,
        });

        await expect(
          client.waitForDAG('dag-123', 'exec-123', { pollInterval: 50 })
        ).rejects.toThrow('DAG dag-123 execution exec-123 failed');
      });
    });

    describe('runDAG', () => {
      it('should start and wait for DAG completion', async () => {
        const startedExecution: DAGExecution = {
          id: 'exec-456',
          dagId: 'dag-123',
          status: DAGStatus.RUNNING,
          nodeExecutions: [],
          createdAt: '2024-01-01T00:00:00Z',
          updatedAt: '2024-01-01T00:00:00Z',
        };

        mockAxiosInstance.onPost('/api/v1/dags/dag-123/start').reply(200, {
          success: true,
          data: startedExecution,
        });

        let pollCount = 0;
        mockAxiosInstance.onGet('/api/v1/dags/dag-123/executions/exec-456').reply(() => {
          pollCount++;
          const status = pollCount < 2 ? DAGStatus.RUNNING : DAGStatus.COMPLETED;
          return [200, { success: true, data: { ...startedExecution, status } }];
        });

        const result = await client.runDAG('dag-123', { key: 'value' }, { pollInterval: 50 });
        expect(result.status).toBe(DAGStatus.COMPLETED);
      });
    });
  });

  describe('WebSocket Methods', () => {
    it('should get WebSocket client', () => {
      const ws = client.getWebSocket();
      expect(ws).toBeDefined();
      // Calling again should return the same instance
      const ws2 = client.getWebSocket();
      expect(ws2).toBe(ws);
    });

    it('should disconnect WebSocket', () => {
      const ws = client.getWebSocket();
      expect(ws).toBeDefined();
      client.disconnectWebSocket();
      // After disconnect, getWebSocket should create a new instance
      const ws2 = client.getWebSocket();
      expect(ws2).not.toBe(ws);
    });
  });
});
