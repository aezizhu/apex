import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { cn, formatCost, formatTokens, formatDuration, formatDate, getStatusColor, getStatusBgColor, getConfidenceColor, debounce, throttle } from '../lib/utils'

describe('Utility Functions', () => {
  describe('cn', () => {
    it('merges class names', () => { expect(cn('foo', 'bar')).toBe('foo bar') })
    it('handles conditional classes', () => { expect(cn('base', true && 'active', false && 'inactive')).toBe('base active') })
    it('handles undefined and null', () => { expect(cn('base', undefined, null, 'end')).toBe('base end') })
    it('merges tailwind classes', () => { expect(cn('px-4 py-2', 'px-6')).toBe('py-2 px-6') })
  })
  describe('formatCost', () => {
    it('formats very small costs', () => { expect(formatCost(0.001234)).toBe('$0.001234') })
    it('formats small costs', () => { expect(formatCost(0.0567)).toBe('$0.0567') })
    it('formats costs >= 1', () => { expect(formatCost(12.5)).toBe('$12.50') })
    it('handles zero', () => { expect(formatCost(0)).toBe('$0.000000') })
  })
  describe('formatTokens', () => {
    it('formats small numbers', () => { expect(formatTokens(500)).toBe('500') })
    it('formats thousands', () => { expect(formatTokens(5000)).toBe('5.0K') })
    it('formats millions', () => { expect(formatTokens(1500000)).toBe('1.50M') })
    it('handles zero', () => { expect(formatTokens(0)).toBe('0') })
  })
  describe('formatDuration', () => {
    it('formats milliseconds', () => { expect(formatDuration(500)).toBe('500ms') })
    it('formats seconds', () => { expect(formatDuration(5000)).toBe('5.0s') })
    it('formats minutes', () => { expect(formatDuration(120000)).toBe('2.0m') })
    it('handles zero', () => { expect(formatDuration(0)).toBe('0ms') })
  })
  describe('formatDate', () => {
    it('formats date string', () => { expect(formatDate('2024-01-15T10:30:00Z')).toBeTruthy() })
    it('formats Date object', () => { expect(formatDate(new Date('2024-06-15T14:30:00Z'))).toBeTruthy() })
  })
  describe('getStatusColor', () => {
    it('idle', () => { expect(getStatusColor('idle')).toBe('text-gray-400') })
    it('busy/running', () => { expect(getStatusColor('busy')).toBe('text-blue-500'); expect(getStatusColor('running')).toBe('text-blue-500') })
    it('completed', () => { expect(getStatusColor('completed')).toBe('text-green-500') })
    it('error/failed', () => { expect(getStatusColor('error')).toBe('text-red-500') })
    it('paused/pending', () => { expect(getStatusColor('paused')).toBe('text-yellow-500') })
    it('unknown', () => { expect(getStatusColor('unknown')).toBe('text-gray-400') })
  })
  describe('getStatusBgColor', () => {
    it('completed', () => { expect(getStatusBgColor('completed')).toBe('bg-green-500/10') })
    it('error', () => { expect(getStatusBgColor('error')).toBe('bg-red-500/10') })
    it('unknown', () => { expect(getStatusBgColor('xyz')).toBe('bg-gray-500/10') })
  })
  describe('getConfidenceColor', () => {
    it('high', () => { expect(getConfidenceColor(0.95)).toBe('#1e40af') })
    it('medium', () => { expect(getConfidenceColor(0.75)).toBe('#3b82f6') })
    it('low-medium', () => { expect(getConfidenceColor(0.55)).toBe('#f59e0b') })
    it('low', () => { expect(getConfidenceColor(0.3)).toBe('#ef4444') })
  })
  describe('debounce', () => {
    beforeEach(() => { vi.useFakeTimers() })
    afterEach(() => { vi.useRealTimers() })
    it('delays execution', () => { const fn = vi.fn(); const d = debounce(fn, 100); d(); expect(fn).not.toHaveBeenCalled(); vi.advanceTimersByTime(100); expect(fn).toHaveBeenCalledTimes(1) })
    it('resets timer', () => { const fn = vi.fn(); const d = debounce(fn, 100); d(); vi.advanceTimersByTime(50); d(); vi.advanceTimersByTime(50); expect(fn).not.toHaveBeenCalled(); vi.advanceTimersByTime(50); expect(fn).toHaveBeenCalledTimes(1) })
  })
  describe('throttle', () => {
    beforeEach(() => { vi.useFakeTimers() })
    afterEach(() => { vi.useRealTimers() })
    it('executes immediately', () => { const fn = vi.fn(); const t = throttle(fn, 100); t(); expect(fn).toHaveBeenCalledTimes(1) })
    it('ignores within window', () => { const fn = vi.fn(); const t = throttle(fn, 100); t(); t(); t(); expect(fn).toHaveBeenCalledTimes(1) })
    it('allows after window', () => { const fn = vi.fn(); const t = throttle(fn, 100); t(); vi.advanceTimersByTime(100); t(); expect(fn).toHaveBeenCalledTimes(2) })
  })
})
