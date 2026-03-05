/**
 * Project Apex TypeScript SDK - Custom Error Classes
 */

import type { TaskError } from './types';

/**
 * Base error class for all Apex SDK errors
 */
export class ApexError extends Error {
  public readonly code: string;
  public readonly details?: Record<string, unknown>;
  public readonly timestamp: Date;

  constructor(message: string, code: string, details?: Record<string, unknown>) {
    super(message);
    this.name = 'ApexError';
    this.code = code;
    this.details = details;
    this.timestamp = new Date();
    Object.setPrototypeOf(this, ApexError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      name: this.name,
      message: this.message,
      code: this.code,
      details: this.details,
      timestamp: this.timestamp.toISOString(),
    };
  }
}

/**
 * Error thrown when an API request fails
 */
export class ApiRequestError extends ApexError {
  public readonly statusCode: number;
  public readonly url: string;
  public readonly method: string;
  public readonly responseBody?: unknown;

  constructor(
    message: string,
    statusCode: number,
    url: string,
    method: string,
    code: string = 'API_REQUEST_ERROR',
    responseBody?: unknown,
    details?: Record<string, unknown>
  ) {
    super(message, code, details);
    this.name = 'ApiRequestError';
    this.statusCode = statusCode;
    this.url = url;
    this.method = method;
    this.responseBody = responseBody;
    Object.setPrototypeOf(this, ApiRequestError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      statusCode: this.statusCode,
      url: this.url,
      method: this.method,
      responseBody: this.responseBody,
    };
  }
}

/**
 * Error thrown when authentication fails
 */
export class AuthenticationError extends ApexError {
  constructor(message: string = 'Authentication failed', details?: Record<string, unknown>) {
    super(message, 'AUTHENTICATION_ERROR', details);
    this.name = 'AuthenticationError';
    Object.setPrototypeOf(this, AuthenticationError.prototype);
  }
}

/**
 * Error thrown when authorization fails
 */
export class AuthorizationError extends ApexError {
  public readonly resource?: string;
  public readonly action?: string;

  constructor(
    message: string = 'Authorization denied',
    resource?: string,
    action?: string,
    details?: Record<string, unknown>
  ) {
    super(message, 'AUTHORIZATION_ERROR', details);
    this.name = 'AuthorizationError';
    this.resource = resource;
    this.action = action;
    Object.setPrototypeOf(this, AuthorizationError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      resource: this.resource,
      action: this.action,
    };
  }
}

/**
 * Error thrown when a resource is not found
 */
export class NotFoundError extends ApexError {
  public readonly resourceType: string;
  public readonly resourceId: string;

  constructor(
    resourceType: string,
    resourceId: string,
    message?: string,
    details?: Record<string, unknown>
  ) {
    super(message ?? `${resourceType} with id '${resourceId}' not found`, 'NOT_FOUND', details);
    this.name = 'NotFoundError';
    this.resourceType = resourceType;
    this.resourceId = resourceId;
    Object.setPrototypeOf(this, NotFoundError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      resourceType: this.resourceType,
      resourceId: this.resourceId,
    };
  }
}

/**
 * Error thrown when validation fails
 */
export class ValidationError extends ApexError {
  public readonly validationErrors: ValidationFieldError[];

  constructor(
    message: string = 'Validation failed',
    validationErrors: ValidationFieldError[] = [],
    details?: Record<string, unknown>
  ) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
    this.validationErrors = validationErrors;
    Object.setPrototypeOf(this, ValidationError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      validationErrors: this.validationErrors,
    };
  }
}

export interface ValidationFieldError {
  field: string;
  message: string;
  value?: unknown;
}

/**
 * Error thrown when rate limit is exceeded
 */
export class RateLimitError extends ApexError {
  public readonly retryAfter?: number;
  public readonly limit?: number;
  public readonly remaining?: number;

  constructor(
    message: string = 'Rate limit exceeded',
    retryAfter?: number,
    limit?: number,
    remaining?: number,
    details?: Record<string, unknown>
  ) {
    super(message, 'RATE_LIMIT_EXCEEDED', details);
    this.name = 'RateLimitError';
    this.retryAfter = retryAfter;
    this.limit = limit;
    this.remaining = remaining;
    Object.setPrototypeOf(this, RateLimitError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      retryAfter: this.retryAfter,
      limit: this.limit,
      remaining: this.remaining,
    };
  }
}

/**
 * Error thrown when a request times out
 */
export class TimeoutError extends ApexError {
  public readonly timeoutMs: number;

  constructor(
    message: string = 'Request timed out',
    timeoutMs: number,
    details?: Record<string, unknown>
  ) {
    super(message, 'TIMEOUT', details);
    this.name = 'TimeoutError';
    this.timeoutMs = timeoutMs;
    Object.setPrototypeOf(this, TimeoutError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      timeoutMs: this.timeoutMs,
    };
  }
}

/**
 * Error thrown when WebSocket connection fails
 */
export class WebSocketError extends ApexError {
  public readonly closeCode?: number;
  public readonly closeReason?: string;

  constructor(
    message: string = 'WebSocket error',
    closeCode?: number,
    closeReason?: string,
    details?: Record<string, unknown>
  ) {
    super(message, 'WEBSOCKET_ERROR', details);
    this.name = 'WebSocketError';
    this.closeCode = closeCode;
    this.closeReason = closeReason;
    Object.setPrototypeOf(this, WebSocketError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      closeCode: this.closeCode,
      closeReason: this.closeReason,
    };
  }
}

/**
 * Error thrown when a task fails
 */
export class TaskExecutionError extends ApexError {
  public readonly taskId: string;
  public readonly taskError: TaskError;

  constructor(
    taskId: string,
    taskError: TaskError,
    message?: string,
    details?: Record<string, unknown>
  ) {
    super(
      message ?? `Task '${taskId}' failed: ${taskError.message}`,
      taskError.code,
      details
    );
    this.name = 'TaskExecutionError';
    this.taskId = taskId;
    this.taskError = taskError;
    Object.setPrototypeOf(this, TaskExecutionError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      taskId: this.taskId,
      taskError: this.taskError,
    };
  }
}

/**
 * Error thrown when maximum retries are exceeded
 */
export class MaxRetriesExceededError extends ApexError {
  public readonly attempts: number;
  public readonly lastError?: Error;

  constructor(
    attempts: number,
    lastError?: Error,
    message?: string,
    details?: Record<string, unknown>
  ) {
    super(
      message ?? `Maximum retries (${attempts}) exceeded`,
      'MAX_RETRIES_EXCEEDED',
      details
    );
    this.name = 'MaxRetriesExceededError';
    this.attempts = attempts;
    this.lastError = lastError;
    Object.setPrototypeOf(this, MaxRetriesExceededError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      attempts: this.attempts,
      lastError: this.lastError
        ? {
            name: this.lastError.name,
            message: this.lastError.message,
          }
        : undefined,
    };
  }
}

/**
 * Error thrown when DAG execution fails
 */
export class DAGExecutionError extends ApexError {
  public readonly dagId: string;
  public readonly nodeId?: string;
  public readonly dagError?: TaskError;

  constructor(
    dagId: string,
    nodeId?: string,
    dagError?: TaskError,
    message?: string,
    details?: Record<string, unknown>
  ) {
    super(
      message ?? `DAG '${dagId}' execution failed${nodeId ? ` at node '${nodeId}'` : ''}`,
      dagError?.code ?? 'DAG_EXECUTION_ERROR',
      details
    );
    this.name = 'DAGExecutionError';
    this.dagId = dagId;
    this.nodeId = nodeId;
    this.dagError = dagError;
    Object.setPrototypeOf(this, DAGExecutionError.prototype);
  }

  toJSON(): Record<string, unknown> {
    return {
      ...super.toJSON(),
      dagId: this.dagId,
      nodeId: this.nodeId,
      dagError: this.dagError,
    };
  }
}

/**
 * Parse API error response into appropriate error class
 */
export function parseApiError(
  statusCode: number,
  url: string,
  method: string,
  responseBody?: unknown
): ApexError {
  const body = responseBody as Record<string, unknown> | undefined;
  const errorData = body?.['error'] as Record<string, unknown> | undefined;
  const message = (errorData?.['message'] as string | undefined) ?? 'Unknown error';
  const code = (errorData?.['code'] as string | undefined) ?? 'UNKNOWN_ERROR';
  const details = errorData?.['details'] as Record<string, unknown> | undefined;

  switch (statusCode) {
    case 400:
      if (code === 'VALIDATION_ERROR') {
        const validationErrors = (errorData?.['validationErrors'] as ValidationFieldError[]) ?? [];
        return new ValidationError(message, validationErrors, details);
      }
      return new ApiRequestError(message, statusCode, url, method, code, responseBody, details);

    case 401:
      return new AuthenticationError(message, details);

    case 403:
      return new AuthorizationError(
        message,
        errorData?.['resource'] as string | undefined,
        errorData?.['action'] as string | undefined,
        details
      );

    case 404:
      return new NotFoundError(
        (errorData?.['resourceType'] as string | undefined) ?? 'Resource',
        (errorData?.['resourceId'] as string | undefined) ?? 'unknown',
        message,
        details
      );

    case 429:
      return new RateLimitError(
        message,
        errorData?.['retryAfter'] as number | undefined,
        errorData?.['limit'] as number | undefined,
        errorData?.['remaining'] as number | undefined,
        details
      );

    case 408:
      return new TimeoutError(
        message,
        (errorData?.['timeoutMs'] as number | undefined) ?? 0,
        details
      );

    default:
      return new ApiRequestError(message, statusCode, url, method, code, responseBody, details);
  }
}
