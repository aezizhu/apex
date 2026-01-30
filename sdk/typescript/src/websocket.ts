/**
 * Project Apex TypeScript SDK - WebSocket Client
 */

import WebSocket from 'ws';
import { EventEmitter } from 'events';
import {
  WebSocketEventType,
  WebSocketMessage,
  WebSocketSubscription,
  TaskCreatedPayload,
  TaskUpdatedPayload,
  TaskCompletedPayload,
  TaskFailedPayload,
  AgentStatusChangedPayload,
  DAGStartedPayload,
  DAGCompletedPayload,
  DAGFailedPayload,
  ApprovalRequiredPayload,
  ApprovalResolvedPayload,
  LogMessagePayload,
  HeartbeatPayload,
} from './types';
import { WebSocketError } from './errors';

// ============================================================================
// Types
// ============================================================================

export interface ApexWebSocketConfig {
  url: string;
  apiKey?: string;
  autoReconnect?: boolean;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
  heartbeatInterval?: number;
  heartbeatTimeout?: number;
}

export interface WebSocketEventMap {
  open: () => void;
  close: (code: number, reason: string) => void;
  error: (error: WebSocketError) => void;
  reconnecting: (attempt: number) => void;
  reconnected: () => void;
  message: (message: WebSocketMessage) => void;
  [WebSocketEventType.TASK_CREATED]: (payload: TaskCreatedPayload) => void;
  [WebSocketEventType.TASK_UPDATED]: (payload: TaskUpdatedPayload) => void;
  [WebSocketEventType.TASK_COMPLETED]: (payload: TaskCompletedPayload) => void;
  [WebSocketEventType.TASK_FAILED]: (payload: TaskFailedPayload) => void;
  [WebSocketEventType.AGENT_STATUS_CHANGED]: (payload: AgentStatusChangedPayload) => void;
  [WebSocketEventType.DAG_STARTED]: (payload: DAGStartedPayload) => void;
  [WebSocketEventType.DAG_COMPLETED]: (payload: DAGCompletedPayload) => void;
  [WebSocketEventType.DAG_FAILED]: (payload: DAGFailedPayload) => void;
  [WebSocketEventType.APPROVAL_REQUIRED]: (payload: ApprovalRequiredPayload) => void;
  [WebSocketEventType.APPROVAL_RESOLVED]: (payload: ApprovalResolvedPayload) => void;
  [WebSocketEventType.LOG_MESSAGE]: (payload: LogMessagePayload) => void;
  [WebSocketEventType.HEARTBEAT]: (payload: HeartbeatPayload) => void;
}

type EventCallback<K extends keyof WebSocketEventMap> = WebSocketEventMap[K];

// ============================================================================
// WebSocket Client
// ============================================================================

export class ApexWebSocket extends EventEmitter {
  private readonly config: Required<ApexWebSocketConfig>;
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private heartbeatTimer: NodeJS.Timeout | null = null;
  private heartbeatTimeoutTimer: NodeJS.Timeout | null = null;
  private subscriptions: Map<string, WebSocketSubscription> = new Map();
  private isClosing = false;
  private connectionId: string | null = null;

  constructor(config: ApexWebSocketConfig) {
    super();
    this.config = {
      url: config.url,
      apiKey: config.apiKey ?? '',
      autoReconnect: config.autoReconnect ?? true,
      reconnectInterval: config.reconnectInterval ?? 1000,
      maxReconnectAttempts: config.maxReconnectAttempts ?? 10,
      heartbeatInterval: config.heartbeatInterval ?? 30000,
      heartbeatTimeout: config.heartbeatTimeout ?? 10000,
    };
  }

  /**
   * Connect to the WebSocket server
   */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        resolve();
        return;
      }

      this.isClosing = false;
      const url = new URL(this.config.url);

      if (this.config.apiKey) {
        url.searchParams.set('apiKey', this.config.apiKey);
      }

      try {
        this.ws = new WebSocket(url.toString());
      } catch (error) {
        const wsError = new WebSocketError(
          `Failed to create WebSocket connection: ${(error as Error).message}`
        );
        reject(wsError);
        return;
      }

      const onOpen = (): void => {
        this.reconnectAttempts = 0;
        this.startHeartbeat();
        this.resubscribeAll();
        this.emit('open');
        resolve();
      };

      const onError = (event: WebSocket.ErrorEvent): void => {
        const wsError = new WebSocketError(
          `WebSocket error: ${event.message ?? 'Unknown error'}`
        );
        this.emit('error', wsError);
        if (this.ws?.readyState !== WebSocket.OPEN) {
          reject(wsError);
        }
      };

      const onClose = (event: WebSocket.CloseEvent): void => {
        this.stopHeartbeat();
        this.emit('close', event.code, event.reason);

        if (!this.isClosing && this.config.autoReconnect) {
          this.attemptReconnect();
        }
      };

      const onMessage = (event: WebSocket.MessageEvent): void => {
        this.handleMessage(event.data);
      };

      this.ws.once('open', onOpen);
      this.ws.once('error', onError);
      this.ws.on('close', onClose);
      this.ws.on('message', onMessage);
    });
  }

  /**
   * Disconnect from the WebSocket server
   */
  disconnect(): void {
    this.isClosing = true;
    this.stopHeartbeat();
    this.clearReconnectTimer();

    if (this.ws) {
      this.ws.close(1000, 'Client disconnecting');
      this.ws = null;
    }

    this.connectionId = null;
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  /**
   * Get current connection ID
   */
  getConnectionId(): string | null {
    return this.connectionId;
  }

  /**
   * Subscribe to specific events
   */
  subscribe(subscription: WebSocketSubscription): string {
    const subscriptionId = this.generateSubscriptionId();
    this.subscriptions.set(subscriptionId, subscription);

    if (this.isConnected()) {
      this.sendSubscription(subscriptionId, subscription);
    }

    return subscriptionId;
  }

  /**
   * Unsubscribe from events
   */
  unsubscribe(subscriptionId: string): void {
    this.subscriptions.delete(subscriptionId);

    if (this.isConnected()) {
      this.send({
        type: 'unsubscribe',
        subscriptionId,
      });
    }
  }

  /**
   * Subscribe to task events
   */
  subscribeToTask(taskId: string): string {
    return this.subscribe({
      event: [
        WebSocketEventType.TASK_UPDATED,
        WebSocketEventType.TASK_COMPLETED,
        WebSocketEventType.TASK_FAILED,
        WebSocketEventType.LOG_MESSAGE,
      ],
      filter: { taskId },
    });
  }

  /**
   * Subscribe to agent events
   */
  subscribeToAgent(agentId: string): string {
    return this.subscribe({
      event: WebSocketEventType.AGENT_STATUS_CHANGED,
      filter: { agentId },
    });
  }

  /**
   * Subscribe to DAG events
   */
  subscribeToDAG(dagId: string): string {
    return this.subscribe({
      event: [
        WebSocketEventType.DAG_STARTED,
        WebSocketEventType.DAG_COMPLETED,
        WebSocketEventType.DAG_FAILED,
      ],
      filter: { dagId },
    });
  }

  /**
   * Subscribe to all approval events
   */
  subscribeToApprovals(): string {
    return this.subscribe({
      event: [
        WebSocketEventType.APPROVAL_REQUIRED,
        WebSocketEventType.APPROVAL_RESOLVED,
      ],
    });
  }

  /**
   * Add typed event listener
   */
  on<K extends keyof WebSocketEventMap>(
    event: K,
    listener: EventCallback<K>
  ): this {
    return super.on(event, listener as (...args: unknown[]) => void);
  }

  /**
   * Add one-time typed event listener
   */
  once<K extends keyof WebSocketEventMap>(
    event: K,
    listener: EventCallback<K>
  ): this {
    return super.once(event, listener as (...args: unknown[]) => void);
  }

  /**
   * Remove typed event listener
   */
  off<K extends keyof WebSocketEventMap>(
    event: K,
    listener: EventCallback<K>
  ): this {
    return super.off(event, listener as (...args: unknown[]) => void);
  }

  /**
   * Wait for a specific event type with optional filter
   */
  waitFor<T = unknown>(
    eventType: WebSocketEventType,
    filter?: (payload: T) => boolean,
    timeoutMs?: number
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      let timeoutTimer: NodeJS.Timeout | undefined;

      const handler = (payload: T): void => {
        if (!filter || filter(payload)) {
          if (timeoutTimer) {
            clearTimeout(timeoutTimer);
          }
          this.off(eventType, handler as never);
          resolve(payload);
        }
      };

      this.on(eventType, handler as never);

      if (timeoutMs) {
        timeoutTimer = setTimeout(() => {
          this.off(eventType, handler as never);
          reject(new WebSocketError(`Timeout waiting for event: ${eventType}`));
        }, timeoutMs);
      }
    });
  }

  // ============================================================================
  // Private Methods
  // ============================================================================

  private handleMessage(data: WebSocket.Data): void {
    try {
      const message = JSON.parse(data.toString()) as WebSocketMessage;

      // Reset heartbeat timeout on any message
      this.resetHeartbeatTimeout();

      // Emit generic message event
      this.emit('message', message);

      // Handle heartbeat
      if (message.type === WebSocketEventType.HEARTBEAT) {
        const payload = message.payload as HeartbeatPayload;
        this.connectionId = payload.connectionId;
        this.emit(WebSocketEventType.HEARTBEAT, payload);
        return;
      }

      // Emit specific event type
      this.emit(message.type, message.payload);
    } catch (error) {
      this.emit(
        'error',
        new WebSocketError(`Failed to parse message: ${(error as Error).message}`)
      );
    }
  }

  private send(data: unknown): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
    }
  }

  private sendSubscription(subscriptionId: string, subscription: WebSocketSubscription): void {
    this.send({
      type: 'subscribe',
      subscriptionId,
      ...subscription,
    });
  }

  private resubscribeAll(): void {
    for (const [id, subscription] of this.subscriptions) {
      this.sendSubscription(id, subscription);
    }
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      this.emit(
        'error',
        new WebSocketError(
          `Max reconnection attempts (${this.config.maxReconnectAttempts}) exceeded`
        )
      );
      return;
    }

    this.reconnectAttempts++;
    const delay = this.calculateReconnectDelay();

    this.emit('reconnecting', this.reconnectAttempts);

    this.reconnectTimer = setTimeout(() => {
      this.connect()
        .then(() => {
          this.emit('reconnected');
        })
        .catch(() => {
          // Error already emitted in connect()
        });
    }, delay);
  }

  private calculateReconnectDelay(): number {
    // Exponential backoff with jitter
    const exponentialDelay = this.config.reconnectInterval * Math.pow(2, this.reconnectAttempts - 1);
    const jitter = Math.random() * 1000;
    return Math.min(exponentialDelay + jitter, 30000); // Max 30 seconds
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      this.send({ type: 'ping' });
      this.startHeartbeatTimeout();
    }, this.config.heartbeatInterval);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    this.clearHeartbeatTimeout();
  }

  private startHeartbeatTimeout(): void {
    this.clearHeartbeatTimeout();
    this.heartbeatTimeoutTimer = setTimeout(() => {
      this.emit(
        'error',
        new WebSocketError('Heartbeat timeout - server not responding')
      );
      // Force reconnect
      if (this.ws) {
        this.ws.close(4000, 'Heartbeat timeout');
      }
    }, this.config.heartbeatTimeout);
  }

  private clearHeartbeatTimeout(): void {
    if (this.heartbeatTimeoutTimer) {
      clearTimeout(this.heartbeatTimeoutTimer);
      this.heartbeatTimeoutTimer = null;
    }
  }

  private resetHeartbeatTimeout(): void {
    this.clearHeartbeatTimeout();
  }

  private generateSubscriptionId(): string {
    return `sub_${Date.now()}_${Math.random().toString(36).substring(2, 11)}`;
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createApexWebSocket(config: ApexWebSocketConfig): ApexWebSocket {
  return new ApexWebSocket(config);
}
