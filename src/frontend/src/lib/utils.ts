import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

// ═══════════════════════════════════════════════════════════════════════════════
// Class Name Utility
// ═══════════════════════════════════════════════════════════════════════════════

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Formatting Utilities
// ═══════════════════════════════════════════════════════════════════════════════

export function formatCost(cost: number): string {
  if (cost < 0.01) {
    return `$${cost.toFixed(6)}`
  }
  if (cost < 1) {
    return `$${cost.toFixed(4)}`
  }
  return `$${cost.toFixed(2)}`
}

export function formatTokens(tokens: number): string {
  if (tokens < 1000) {
    return String(tokens)
  }
  if (tokens < 1_000_000) {
    return `${(tokens / 1000).toFixed(1)}K`
  }
  return `${(tokens / 1_000_000).toFixed(2)}M`
}

export function formatDuration(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`
  }
  if (ms < 60_000) {
    return `${(ms / 1000).toFixed(1)}s`
  }
  return `${(ms / 60_000).toFixed(1)}m`
}

export function formatDate(date: string | Date): string {
  const d = typeof date === 'string' ? new Date(date) : date
  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Status Color Utilities
// ═══════════════════════════════════════════════════════════════════════════════

export function getStatusColor(status: string): string {
  switch (status) {
    case 'idle':
      return 'text-gray-400'
    case 'busy':
    case 'running':
      return 'text-blue-500'
    case 'completed':
    case 'ready':
      return 'text-green-500'
    case 'error':
    case 'failed':
      return 'text-red-500'
    case 'paused':
    case 'pending':
      return 'text-yellow-500'
    case 'cancelled':
      return 'text-gray-500'
    default:
      return 'text-gray-400'
  }
}

export function getStatusBgColor(status: string): string {
  switch (status) {
    case 'completed':
    case 'ready':
      return 'bg-green-500/10'
    case 'running':
    case 'busy':
      return 'bg-blue-500/10'
    case 'error':
    case 'failed':
      return 'bg-red-500/10'
    case 'paused':
    case 'pending':
      return 'bg-yellow-500/10'
    case 'cancelled':
      return 'bg-gray-500/10'
    default:
      return 'bg-gray-500/10'
  }
}

export function getConfidenceColor(confidence: number): string {
  if (confidence >= 0.9) return '#1e40af' // blue-800
  if (confidence >= 0.7) return '#3b82f6' // blue-500
  if (confidence >= 0.5) return '#f59e0b' // amber-500
  return '#ef4444' // red-500
}

// ═══════════════════════════════════════════════════════════════════════════════
// Performance Utilities
// ═══════════════════════════════════════════════════════════════════════════════

export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null

  return (...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    timeoutId = setTimeout(() => {
      fn(...args)
    }, delay)
  }
}

export function throttle<T extends (...args: unknown[]) => unknown>(
  fn: T,
  limit: number
): (...args: Parameters<T>) => void {
  let inThrottle = false

  return (...args: Parameters<T>) => {
    if (!inThrottle) {
      fn(...args)
      inThrottle = true
      setTimeout(() => {
        inThrottle = false
      }, limit)
    }
  }
}
