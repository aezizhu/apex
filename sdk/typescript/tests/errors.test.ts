/**
 * Tests for error classes
 */

import {
  ApexError,
  ApiRequestError,
  AuthenticationError,
  AuthorizationError,
  NotFoundError,
  ValidationError,
  RateLimitError,
  TimeoutError,
  WebSocketError,
  TaskExecutionError,
  MaxRetriesExceededError,
  DAGExecutionError,
  parseApiError,
  ValidationFieldError,
} from '../src';

describe('Error Classes', () => {
  describe('ApexError', () => {
    it('should create error with message and code', () => {
      const error = new ApexError('Test error', 'TEST_ERROR');

      expect(error.message).toBe('Test error');
      expect(error.code).toBe('TEST_ERROR');
      expect(error.name).toBe('ApexError');
      expect(error.timestamp).toBeInstanceOf(Date);
    });

    it('should create error with details', () => {
      const details = { key: 'value' };
      const error = new ApexError('Test error', 'TEST_ERROR', details);

      expect(error.details).toEqual(details);
    });

    it('should be instance of Error', () => {
      const error = new ApexError('Test error', 'TEST_ERROR');

      expect(error).toBeInstanceOf(Error);
      expect(error).toBeInstanceOf(ApexError);
    });

    it('should serialize to JSON', () => {
      const error = new ApexError('Test error', 'TEST_ERROR', { key: 'value' });
      const json = error.toJSON();

      expect(json).toEqual({
        name: 'ApexError',
        message: 'Test error',
        code: 'TEST_ERROR',
        details: { key: 'value' },
        timestamp: expect.any(String),
      });
    });
  });

  describe('ApiRequestError', () => {
    it('should create error with HTTP details', () => {
      const error = new ApiRequestError(
        'Request failed',
        500,
        '/api/v1/tasks',
        'GET'
      );

      expect(error.message).toBe('Request failed');
      expect(error.statusCode).toBe(500);
      expect(error.url).toBe('/api/v1/tasks');
      expect(error.method).toBe('GET');
      expect(error.name).toBe('ApiRequestError');
    });

    it('should create error with custom code', () => {
      const error = new ApiRequestError(
        'Request failed',
        500,
        '/api/v1/tasks',
        'GET',
        'CUSTOM_ERROR'
      );

      expect(error.code).toBe('CUSTOM_ERROR');
    });

    it('should create error with response body', () => {
      const responseBody = { error: { message: 'Server error' } };
      const error = new ApiRequestError(
        'Request failed',
        500,
        '/api/v1/tasks',
        'GET',
        'API_REQUEST_ERROR',
        responseBody
      );

      expect(error.responseBody).toEqual(responseBody);
    });

    it('should serialize to JSON with HTTP details', () => {
      const error = new ApiRequestError(
        'Request failed',
        404,
        '/api/v1/tasks/123',
        'GET'
      );
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          statusCode: 404,
          url: '/api/v1/tasks/123',
          method: 'GET',
        })
      );
    });
  });

  describe('AuthenticationError', () => {
    it('should create error with default message', () => {
      const error = new AuthenticationError();

      expect(error.message).toBe('Authentication failed');
      expect(error.code).toBe('AUTHENTICATION_ERROR');
      expect(error.name).toBe('AuthenticationError');
    });

    it('should create error with custom message', () => {
      const error = new AuthenticationError('Invalid token');

      expect(error.message).toBe('Invalid token');
    });

    it('should be instance of ApexError', () => {
      const error = new AuthenticationError();

      expect(error).toBeInstanceOf(ApexError);
      expect(error).toBeInstanceOf(AuthenticationError);
    });
  });

  describe('AuthorizationError', () => {
    it('should create error with default message', () => {
      const error = new AuthorizationError();

      expect(error.message).toBe('Authorization denied');
      expect(error.code).toBe('AUTHORIZATION_ERROR');
      expect(error.name).toBe('AuthorizationError');
    });

    it('should create error with resource and action', () => {
      const error = new AuthorizationError(
        'Cannot delete task',
        'task',
        'delete'
      );

      expect(error.message).toBe('Cannot delete task');
      expect(error.resource).toBe('task');
      expect(error.action).toBe('delete');
    });

    it('should serialize to JSON with resource and action', () => {
      const error = new AuthorizationError(
        'Cannot delete task',
        'task',
        'delete'
      );
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          resource: 'task',
          action: 'delete',
        })
      );
    });
  });

  describe('NotFoundError', () => {
    it('should create error with resource type and ID', () => {
      const error = new NotFoundError('Task', 'task-123');

      expect(error.message).toBe("Task with id 'task-123' not found");
      expect(error.code).toBe('NOT_FOUND');
      expect(error.resourceType).toBe('Task');
      expect(error.resourceId).toBe('task-123');
      expect(error.name).toBe('NotFoundError');
    });

    it('should create error with custom message', () => {
      const error = new NotFoundError('Task', 'task-123', 'Custom not found message');

      expect(error.message).toBe('Custom not found message');
    });

    it('should serialize to JSON with resource info', () => {
      const error = new NotFoundError('Task', 'task-123');
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          resourceType: 'Task',
          resourceId: 'task-123',
        })
      );
    });
  });

  describe('ValidationError', () => {
    it('should create error with default message', () => {
      const error = new ValidationError();

      expect(error.message).toBe('Validation failed');
      expect(error.code).toBe('VALIDATION_ERROR');
      expect(error.validationErrors).toEqual([]);
      expect(error.name).toBe('ValidationError');
    });

    it('should create error with validation errors', () => {
      const validationErrors: ValidationFieldError[] = [
        { field: 'name', message: 'Name is required' },
        { field: 'priority', message: 'Invalid priority', value: 'invalid' },
      ];
      const error = new ValidationError('Validation failed', validationErrors);

      expect(error.validationErrors).toEqual(validationErrors);
    });

    it('should serialize to JSON with validation errors', () => {
      const validationErrors: ValidationFieldError[] = [
        { field: 'name', message: 'Name is required' },
      ];
      const error = new ValidationError('Validation failed', validationErrors);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          validationErrors,
        })
      );
    });
  });

  describe('RateLimitError', () => {
    it('should create error with default message', () => {
      const error = new RateLimitError();

      expect(error.message).toBe('Rate limit exceeded');
      expect(error.code).toBe('RATE_LIMIT_EXCEEDED');
      expect(error.name).toBe('RateLimitError');
    });

    it('should create error with rate limit info', () => {
      const error = new RateLimitError('Too many requests', 60, 100, 0);

      expect(error.message).toBe('Too many requests');
      expect(error.retryAfter).toBe(60);
      expect(error.limit).toBe(100);
      expect(error.remaining).toBe(0);
    });

    it('should serialize to JSON with rate limit info', () => {
      const error = new RateLimitError('Too many requests', 60, 100, 0);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          retryAfter: 60,
          limit: 100,
          remaining: 0,
        })
      );
    });
  });

  describe('TimeoutError', () => {
    it('should create error with timeout duration', () => {
      const error = new TimeoutError('Request timed out', 5000);

      expect(error.message).toBe('Request timed out');
      expect(error.code).toBe('TIMEOUT');
      expect(error.timeoutMs).toBe(5000);
      expect(error.name).toBe('TimeoutError');
    });

    it('should create error with default message', () => {
      const error = new TimeoutError(undefined, 3000);

      expect(error.message).toBe('Request timed out');
    });

    it('should serialize to JSON with timeout', () => {
      const error = new TimeoutError('Request timed out', 5000);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          timeoutMs: 5000,
        })
      );
    });
  });

  describe('WebSocketError', () => {
    it('should create error with default message', () => {
      const error = new WebSocketError();

      expect(error.message).toBe('WebSocket error');
      expect(error.code).toBe('WEBSOCKET_ERROR');
      expect(error.name).toBe('WebSocketError');
    });

    it('should create error with close code and reason', () => {
      const error = new WebSocketError('Connection closed', 1006, 'Connection lost');

      expect(error.message).toBe('Connection closed');
      expect(error.closeCode).toBe(1006);
      expect(error.closeReason).toBe('Connection lost');
    });

    it('should serialize to JSON with close info', () => {
      const error = new WebSocketError('Connection closed', 1006, 'Connection lost');
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          closeCode: 1006,
          closeReason: 'Connection lost',
        })
      );
    });
  });

  describe('TaskExecutionError', () => {
    it('should create error with task ID and error', () => {
      const taskError = {
        code: 'EXECUTION_ERROR',
        message: 'Task execution failed',
      };
      const error = new TaskExecutionError('task-123', taskError);

      expect(error.message).toBe("Task 'task-123' failed: Task execution failed");
      expect(error.code).toBe('EXECUTION_ERROR');
      expect(error.taskId).toBe('task-123');
      expect(error.taskError).toEqual(taskError);
      expect(error.name).toBe('TaskExecutionError');
    });

    it('should create error with custom message', () => {
      const taskError = {
        code: 'EXECUTION_ERROR',
        message: 'Task execution failed',
      };
      const error = new TaskExecutionError('task-123', taskError, 'Custom message');

      expect(error.message).toBe('Custom message');
    });

    it('should serialize to JSON with task info', () => {
      const taskError = {
        code: 'EXECUTION_ERROR',
        message: 'Task execution failed',
        details: { step: 3 },
      };
      const error = new TaskExecutionError('task-123', taskError);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          taskId: 'task-123',
          taskError,
        })
      );
    });
  });

  describe('MaxRetriesExceededError', () => {
    it('should create error with attempts count', () => {
      const error = new MaxRetriesExceededError(3);

      expect(error.message).toBe('Maximum retries (3) exceeded');
      expect(error.code).toBe('MAX_RETRIES_EXCEEDED');
      expect(error.attempts).toBe(3);
      expect(error.name).toBe('MaxRetriesExceededError');
    });

    it('should create error with last error', () => {
      const lastError = new Error('Connection failed');
      const error = new MaxRetriesExceededError(3, lastError);

      expect(error.lastError).toBe(lastError);
    });

    it('should create error with custom message', () => {
      const error = new MaxRetriesExceededError(3, undefined, 'Custom retry message');

      expect(error.message).toBe('Custom retry message');
    });

    it('should serialize to JSON with retry info', () => {
      const lastError = new Error('Connection failed');
      const error = new MaxRetriesExceededError(3, lastError);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          attempts: 3,
          lastError: {
            name: 'Error',
            message: 'Connection failed',
          },
        })
      );
    });

    it('should serialize to JSON without last error', () => {
      const error = new MaxRetriesExceededError(3);
      const json = error.toJSON();

      expect(json.lastError).toBeUndefined();
    });
  });

  describe('DAGExecutionError', () => {
    it('should create error with DAG ID', () => {
      const error = new DAGExecutionError('dag-123');

      expect(error.message).toBe("DAG 'dag-123' execution failed");
      expect(error.code).toBe('DAG_EXECUTION_ERROR');
      expect(error.dagId).toBe('dag-123');
      expect(error.name).toBe('DAGExecutionError');
    });

    it('should create error with node ID', () => {
      const error = new DAGExecutionError('dag-123', 'node-456');

      expect(error.message).toBe("DAG 'dag-123' execution failed at node 'node-456'");
      expect(error.nodeId).toBe('node-456');
    });

    it('should create error with DAG error', () => {
      const dagError = {
        code: 'NODE_TIMEOUT',
        message: 'Node timed out',
      };
      const error = new DAGExecutionError('dag-123', 'node-456', dagError);

      expect(error.code).toBe('NODE_TIMEOUT');
      expect(error.dagError).toEqual(dagError);
    });

    it('should create error with custom message', () => {
      const error = new DAGExecutionError('dag-123', undefined, undefined, 'Custom DAG error');

      expect(error.message).toBe('Custom DAG error');
    });

    it('should serialize to JSON with DAG info', () => {
      const dagError = {
        code: 'NODE_TIMEOUT',
        message: 'Node timed out',
      };
      const error = new DAGExecutionError('dag-123', 'node-456', dagError);
      const json = error.toJSON();

      expect(json).toEqual(
        expect.objectContaining({
          dagId: 'dag-123',
          nodeId: 'node-456',
          dagError,
        })
      );
    });
  });
});

describe('parseApiError', () => {
  describe('400 Bad Request', () => {
    it('should return ValidationError for validation errors', () => {
      const error = parseApiError(400, '/api/v1/tasks', 'POST', {
        error: {
          code: 'VALIDATION_ERROR',
          message: 'Validation failed',
          validationErrors: [
            { field: 'name', message: 'Name is required' },
          ],
        },
      });

      expect(error).toBeInstanceOf(ValidationError);
      expect((error as ValidationError).validationErrors).toHaveLength(1);
    });

    it('should return ApiRequestError for non-validation 400 errors', () => {
      const error = parseApiError(400, '/api/v1/tasks', 'POST', {
        error: {
          code: 'BAD_REQUEST',
          message: 'Invalid request',
        },
      });

      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).statusCode).toBe(400);
    });
  });

  describe('401 Unauthorized', () => {
    it('should return AuthenticationError', () => {
      const error = parseApiError(401, '/api/v1/tasks', 'GET', {
        error: {
          code: 'AUTHENTICATION_ERROR',
          message: 'Invalid API key',
        },
      });

      expect(error).toBeInstanceOf(AuthenticationError);
    });
  });

  describe('403 Forbidden', () => {
    it('should return AuthorizationError', () => {
      const error = parseApiError(403, '/api/v1/tasks/123', 'DELETE', {
        error: {
          code: 'AUTHORIZATION_ERROR',
          message: 'Access denied',
          resource: 'task',
          action: 'delete',
        },
      });

      expect(error).toBeInstanceOf(AuthorizationError);
      expect((error as AuthorizationError).resource).toBe('task');
      expect((error as AuthorizationError).action).toBe('delete');
    });
  });

  describe('404 Not Found', () => {
    it('should return NotFoundError', () => {
      const error = parseApiError(404, '/api/v1/tasks/123', 'GET', {
        error: {
          code: 'NOT_FOUND',
          message: 'Task not found',
          resourceType: 'Task',
          resourceId: '123',
        },
      });

      expect(error).toBeInstanceOf(NotFoundError);
      expect((error as NotFoundError).resourceType).toBe('Task');
      expect((error as NotFoundError).resourceId).toBe('123');
    });

    it('should use default resource type and ID if not provided', () => {
      const error = parseApiError(404, '/api/v1/tasks/123', 'GET', {
        error: {
          code: 'NOT_FOUND',
          message: 'Not found',
        },
      });

      expect(error).toBeInstanceOf(NotFoundError);
      expect((error as NotFoundError).resourceType).toBe('Resource');
      expect((error as NotFoundError).resourceId).toBe('unknown');
    });
  });

  describe('408 Request Timeout', () => {
    it('should return TimeoutError', () => {
      const error = parseApiError(408, '/api/v1/tasks', 'GET', {
        error: {
          code: 'TIMEOUT',
          message: 'Request timeout',
          timeoutMs: 30000,
        },
      });

      expect(error).toBeInstanceOf(TimeoutError);
      expect((error as TimeoutError).timeoutMs).toBe(30000);
    });

    it('should use default timeout if not provided', () => {
      const error = parseApiError(408, '/api/v1/tasks', 'GET', {
        error: {
          code: 'TIMEOUT',
          message: 'Request timeout',
        },
      });

      expect(error).toBeInstanceOf(TimeoutError);
      expect((error as TimeoutError).timeoutMs).toBe(0);
    });
  });

  describe('429 Too Many Requests', () => {
    it('should return RateLimitError', () => {
      const error = parseApiError(429, '/api/v1/tasks', 'GET', {
        error: {
          code: 'RATE_LIMIT_EXCEEDED',
          message: 'Rate limit exceeded',
          retryAfter: 60,
          limit: 100,
          remaining: 0,
        },
      });

      expect(error).toBeInstanceOf(RateLimitError);
      expect((error as RateLimitError).retryAfter).toBe(60);
      expect((error as RateLimitError).limit).toBe(100);
      expect((error as RateLimitError).remaining).toBe(0);
    });
  });

  describe('5xx Server Errors', () => {
    it('should return ApiRequestError for 500 errors', () => {
      const error = parseApiError(500, '/api/v1/tasks', 'GET', {
        error: {
          code: 'INTERNAL_SERVER_ERROR',
          message: 'Internal server error',
        },
      });

      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).statusCode).toBe(500);
    });

    it('should return ApiRequestError for 502 errors', () => {
      const error = parseApiError(502, '/api/v1/tasks', 'GET', {
        error: {
          code: 'BAD_GATEWAY',
          message: 'Bad gateway',
        },
      });

      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).statusCode).toBe(502);
    });

    it('should return ApiRequestError for 503 errors', () => {
      const error = parseApiError(503, '/api/v1/tasks', 'GET', {
        error: {
          code: 'SERVICE_UNAVAILABLE',
          message: 'Service unavailable',
        },
      });

      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).statusCode).toBe(503);
    });
  });

  describe('Unknown Errors', () => {
    it('should return ApiRequestError for unknown status codes', () => {
      const error = parseApiError(418, '/api/v1/tasks', 'GET', {
        error: {
          code: 'TEAPOT',
          message: "I'm a teapot",
        },
      });

      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).statusCode).toBe(418);
    });

    it('should handle missing error data', () => {
      const error = parseApiError(500, '/api/v1/tasks', 'GET', undefined);

      expect(error).toBeInstanceOf(ApiRequestError);
      expect(error.message).toBe('Unknown error');
      expect(error.code).toBe('UNKNOWN_ERROR');
    });

    it('should handle empty error object', () => {
      const error = parseApiError(500, '/api/v1/tasks', 'GET', {});

      expect(error).toBeInstanceOf(ApiRequestError);
      expect(error.message).toBe('Unknown error');
    });
  });

  describe('Error details', () => {
    it('should preserve error details', () => {
      const details = { requestId: 'req-123', trace: 'trace-456' };
      const error = parseApiError(500, '/api/v1/tasks', 'GET', {
        error: {
          code: 'INTERNAL_SERVER_ERROR',
          message: 'Internal server error',
          details,
        },
      });

      expect(error.details).toEqual(details);
    });
  });
});

describe('Error inheritance', () => {
  it('should allow catching ApexError base class', () => {
    const errors = [
      new AuthenticationError(),
      new AuthorizationError(),
      new NotFoundError('Task', '123'),
      new ValidationError(),
      new RateLimitError(),
      new TimeoutError(undefined, 5000),
      new WebSocketError(),
      new TaskExecutionError('task-1', { code: 'ERR', message: 'err' }),
      new MaxRetriesExceededError(3),
      new DAGExecutionError('dag-1'),
    ];

    errors.forEach((error) => {
      expect(error).toBeInstanceOf(ApexError);
      expect(error).toBeInstanceOf(Error);
    });
  });

  it('should allow catching ApiRequestError', () => {
    const error = new ApiRequestError('Error', 500, '/path', 'GET');

    expect(error).toBeInstanceOf(ApexError);
    expect(error).toBeInstanceOf(ApiRequestError);
    expect(error).toBeInstanceOf(Error);
  });

  it('should preserve prototype chain for try/catch', () => {
    try {
      throw new NotFoundError('Task', '123');
    } catch (e) {
      expect(e instanceof NotFoundError).toBe(true);
      expect(e instanceof ApexError).toBe(true);
      expect(e instanceof Error).toBe(true);
    }
  });
});
