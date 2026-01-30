import { describe, it, expect } from 'vitest'
import { cn, formatCurrency, formatNumber, formatDuration, formatPercentage } from '../lib/utils'

describe('Utility Functions', () => {
  describe('cn (classNames)', () => {
    it('should merge class names', () => {
      const result = cn('foo', 'bar')
      expect(result).toBe('foo bar')
    })

    it('should handle conditional classes', () => {
      const result = cn('base', true && 'active', false && 'inactive')
      expect(result).toBe('base active')
    })

    it('should handle undefined and null', () => {
      const result = cn('base', undefined, null, 'end')
      expect(result).toBe('base end')
    })

    it('should merge tailwind classes correctly', () => {
      const result = cn('px-4 py-2', 'px-6')
      expect(result).toBe('py-2 px-6')
    })
  })

  describe('formatCurrency', () => {
    it('should format USD by default', () => {
      const result = formatCurrency(123.45)
      expect(result).toContain('123.45')
      expect(result).toContain('$')
    })

    it('should handle zero', () => {
      const result = formatCurrency(0)
      expect(result).toContain('0')
    })

    it('should handle large numbers', () => {
      const result = formatCurrency(1234567.89)
      expect(result).toContain('1,234,567.89')
    })
  })

  describe('formatNumber', () => {
    it('should format integers', () => {
      const result = formatNumber(1234)
      expect(result).toBe('1,234')
    })

    it('should handle zero', () => {
      const result = formatNumber(0)
      expect(result).toBe('0')
    })

    it('should handle large numbers', () => {
      const result = formatNumber(1000000)
      expect(result).toBe('1,000,000')
    })
  })

  describe('formatDuration', () => {
    it('should format seconds', () => {
      const result = formatDuration(45)
      expect(result).toBe('45s')
    })

    it('should format minutes and seconds', () => {
      const result = formatDuration(125)
      expect(result).toBe('2m 5s')
    })

    it('should format hours', () => {
      const result = formatDuration(3665)
      expect(result).toBe('1h 1m 5s')
    })

    it('should handle zero', () => {
      const result = formatDuration(0)
      expect(result).toBe('0s')
    })
  })

  describe('formatPercentage', () => {
    it('should format percentage', () => {
      const result = formatPercentage(0.75)
      expect(result).toBe('75%')
    })

    it('should handle 100%', () => {
      const result = formatPercentage(1)
      expect(result).toBe('100%')
    })

    it('should handle 0%', () => {
      const result = formatPercentage(0)
      expect(result).toBe('0%')
    })

    it('should handle decimals', () => {
      const result = formatPercentage(0.755)
      expect(result).toMatch(/75\.?5?%/)
    })
  })
})
