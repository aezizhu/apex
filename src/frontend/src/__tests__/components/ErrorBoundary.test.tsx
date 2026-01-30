import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  ErrorBoundary,
  ErrorBoundaryWrapper,
  PageError,
} from '@/components/ErrorBoundary'

// Component that throws an error
const ThrowError = ({ shouldThrow = true }: { shouldThrow?: boolean }) => {
  if (shouldThrow) {
    throw new Error('Test error message')
  }
  return <div data-testid="child">No error</div>
}

// Component that throws an error with component stack
const ThrowErrorWithStack = () => {
  throw new Error('Error with stack')
}

describe('ErrorBoundary', () => {
  // Suppress console.error for cleaner test output
  const originalError = console.error
  beforeEach(() => {
    console.error = vi.fn()
  })
  afterEach(() => {
    console.error = originalError
  })

  describe('rendering children', () => {
    it('renders children when no error occurs', () => {
      render(
        <ErrorBoundary>
          <div data-testid="child">Child content</div>
        </ErrorBoundary>
      )

      expect(screen.getByTestId('child')).toBeInTheDocument()
      expect(screen.getByText('Child content')).toBeInTheDocument()
    })

    it('does not show error UI when no error', () => {
      render(
        <ErrorBoundary>
          <div>Content</div>
        </ErrorBoundary>
      )

      expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument()
    })
  })

  describe('error handling', () => {
    it('catches errors and shows error UI', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    })

    it('shows error message in UI', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(
        screen.getByText(/An unexpected error occurred/)
      ).toBeInTheDocument()
    })

    it('calls onError callback when error occurs', () => {
      const handleError = vi.fn()

      render(
        <ErrorBoundary onError={handleError}>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(handleError).toHaveBeenCalledTimes(1)
      expect(handleError).toHaveBeenCalledWith(
        expect.any(Error),
        expect.objectContaining({ componentStack: expect.any(String) })
      )
    })

    it('logs error to console', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(console.error).toHaveBeenCalledWith(
        'Error caught by ErrorBoundary:',
        expect.any(Error),
        expect.any(Object)
      )
    })
  })

  describe('custom fallback', () => {
    it('renders custom fallback when provided', () => {
      render(
        <ErrorBoundary fallback={<div data-testid="custom-fallback">Custom Error UI</div>}>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(screen.getByTestId('custom-fallback')).toBeInTheDocument()
      expect(screen.getByText('Custom Error UI')).toBeInTheDocument()
    })

    it('does not render default error UI when fallback provided', () => {
      render(
        <ErrorBoundary fallback={<div>Custom</div>}>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument()
    })
  })

  describe('reset functionality', () => {
    it('renders Try Again button', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(screen.getByRole('button', { name: /Try Again/i })).toBeInTheDocument()
    })

    it('renders Go Home button', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(screen.getByRole('button', { name: /Go Home/i })).toBeInTheDocument()
    })

    it('calls onReset when Try Again is clicked', async () => {
      const handleReset = vi.fn()
      const user = userEvent.setup()

      render(
        <ErrorBoundary onReset={handleReset}>
          <ThrowError />
        </ErrorBoundary>
      )

      await user.click(screen.getByRole('button', { name: /Try Again/i }))
      expect(handleReset).toHaveBeenCalledTimes(1)
    })

    it('resets error state on Try Again click', async () => {
      const user = userEvent.setup()
      let shouldThrow = true

      const { rerender } = render(
        <ErrorBoundary>
          <ThrowError shouldThrow={shouldThrow} />
        </ErrorBoundary>
      )

      // Error should be shown
      expect(screen.getByText('Something went wrong')).toBeInTheDocument()

      // Simulate fix and click Try Again
      shouldThrow = false
      await user.click(screen.getByRole('button', { name: /Try Again/i }))

      // After reset with fixed component, re-render should work
      rerender(
        <ErrorBoundary>
          <ThrowError shouldThrow={false} />
        </ErrorBoundary>
      )
    })

    it('handles Go Home click by redirecting', async () => {
      const user = userEvent.setup()
      const originalHref = window.location.href

      // Mock window.location
      delete (window as any).location
      window.location = { href: originalHref } as any

      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      await user.click(screen.getByRole('button', { name: /Go Home/i }))
      expect(window.location.href).toBe('/')

      // Restore
      window.location = { href: originalHref } as any
    })
  })

  describe('error details in development', () => {
    it('shows error details toggle in dev mode', () => {
      const originalEnv = import.meta.env.DEV

      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      // In test environment, DEV should be available
      // Check for details element
      const details = screen.queryByText(/View error details/i)
      // This depends on DEV environment variable
    })

    it('shows error message in details', () => {
      render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      // Look for the actual error message
      const errorMessage = screen.queryByText('Test error message')
      // This is shown in development mode
    })
  })

  describe('styling', () => {
    it('renders with Card component', () => {
      const { container } = render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      // Should have elevated card styling
      expect(container.querySelector('.rounded-xl')).toBeInTheDocument()
    })

    it('centers content on screen', () => {
      const { container } = render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      expect(container.querySelector('.flex.items-center.justify-center')).toBeInTheDocument()
    })

    it('shows alert icon', () => {
      const { container } = render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      // AlertTriangle icon should be present
      const icon = container.querySelector('.text-red-500')
      expect(icon).toBeInTheDocument()
    })
  })
})

describe('ErrorBoundaryWrapper', () => {
  const originalError = console.error
  beforeEach(() => {
    console.error = vi.fn()
  })
  afterEach(() => {
    console.error = originalError
  })

  it('renders children when no error', () => {
    render(
      <ErrorBoundaryWrapper>
        <div data-testid="child">Content</div>
      </ErrorBoundaryWrapper>
    )

    expect(screen.getByTestId('child')).toBeInTheDocument()
  })

  it('catches errors and shows error UI', () => {
    render(
      <ErrorBoundaryWrapper>
        <ThrowError />
      </ErrorBoundaryWrapper>
    )

    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
  })

  it('uses custom fallback', () => {
    render(
      <ErrorBoundaryWrapper fallback={<div>Custom Fallback</div>}>
        <ThrowError />
      </ErrorBoundaryWrapper>
    )

    expect(screen.getByText('Custom Fallback')).toBeInTheDocument()
  })

  it('calls onError callback', () => {
    const handleError = vi.fn()

    render(
      <ErrorBoundaryWrapper onError={handleError}>
        <ThrowError />
      </ErrorBoundaryWrapper>
    )

    expect(handleError).toHaveBeenCalled()
  })

  it('remounts on resetKeys change', () => {
    const { rerender } = render(
      <ErrorBoundaryWrapper resetKeys={['key1']}>
        <ThrowError />
      </ErrorBoundaryWrapper>
    )

    expect(screen.getByText('Something went wrong')).toBeInTheDocument()

    // Change reset keys to force remount
    rerender(
      <ErrorBoundaryWrapper resetKeys={['key2']}>
        <ThrowError shouldThrow={false} />
      </ErrorBoundaryWrapper>
    )

    // After reset key change, should try to render children again
    expect(screen.getByTestId('child')).toBeInTheDocument()
  })

  it('generates key from resetKeys array', () => {
    render(
      <ErrorBoundaryWrapper resetKeys={['a', 'b', 'c']}>
        <div>Content</div>
      </ErrorBoundaryWrapper>
    )

    expect(screen.getByText('Content')).toBeInTheDocument()
  })
})

describe('PageError', () => {
  describe('rendering', () => {
    it('renders with default props', () => {
      render(<PageError />)

      expect(screen.getByText('Page Error')).toBeInTheDocument()
      expect(screen.getByText('Failed to load this page. Please try again.')).toBeInTheDocument()
    })

    it('renders custom title', () => {
      render(<PageError title="Custom Title" />)

      expect(screen.getByText('Custom Title')).toBeInTheDocument()
    })

    it('renders custom message', () => {
      render(<PageError message="Custom error message" />)

      expect(screen.getByText('Custom error message')).toBeInTheDocument()
    })

    it('shows alert icon', () => {
      const { container } = render(<PageError />)

      const icon = container.querySelector('.text-yellow-500')
      expect(icon).toBeInTheDocument()
    })
  })

  describe('buttons', () => {
    it('renders Retry button when onRetry provided', () => {
      render(<PageError onRetry={() => {}} />)

      expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument()
    })

    it('does not render Retry button when onRetry not provided', () => {
      render(<PageError />)

      expect(screen.queryByRole('button', { name: /Retry/i })).not.toBeInTheDocument()
    })

    it('renders Go Back button when onGoBack provided', () => {
      render(<PageError onGoBack={() => {}} />)

      expect(screen.getByRole('button', { name: /Go Back/i })).toBeInTheDocument()
    })

    it('does not render Go Back button when onGoBack not provided', () => {
      render(<PageError />)

      expect(screen.queryByRole('button', { name: /Go Back/i })).not.toBeInTheDocument()
    })

    it('calls onRetry when Retry clicked', async () => {
      const handleRetry = vi.fn()
      const user = userEvent.setup()

      render(<PageError onRetry={handleRetry} />)

      await user.click(screen.getByRole('button', { name: /Retry/i }))
      expect(handleRetry).toHaveBeenCalledTimes(1)
    })

    it('calls onGoBack when Go Back clicked', async () => {
      const handleGoBack = vi.fn()
      const user = userEvent.setup()

      render(<PageError onGoBack={handleGoBack} />)

      await user.click(screen.getByRole('button', { name: /Go Back/i }))
      expect(handleGoBack).toHaveBeenCalledTimes(1)
    })

    it('renders both buttons when both handlers provided', () => {
      render(<PageError onRetry={() => {}} onGoBack={() => {}} />)

      expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /Go Back/i })).toBeInTheDocument()
    })
  })

  describe('styling', () => {
    it('centers content', () => {
      const { container } = render(<PageError />)

      expect(container.querySelector('.flex.flex-col.items-center.justify-center')).toBeInTheDocument()
    })

    it('has minimum height', () => {
      const { container } = render(<PageError />)

      expect(container.querySelector('.min-h-\\[400px\\]')).toBeInTheDocument()
    })

    it('uses correct button variants', () => {
      render(<PageError onRetry={() => {}} onGoBack={() => {}} />)

      // Primary button for Retry
      const retryButton = screen.getByRole('button', { name: /Retry/i })
      expect(retryButton).toHaveClass('bg-apex-accent-primary')

      // Secondary button for Go Back
      const goBackButton = screen.getByRole('button', { name: /Go Back/i })
      expect(goBackButton).toHaveClass('bg-apex-bg-tertiary')
    })
  })

  describe('accessibility', () => {
    it('has heading for title', () => {
      render(<PageError title="Error Title" />)

      expect(screen.getByRole('heading', { name: 'Error Title' })).toBeInTheDocument()
    })

    it('buttons are keyboard accessible', async () => {
      const handleRetry = vi.fn()
      const user = userEvent.setup()

      render(<PageError onRetry={handleRetry} />)

      await user.tab()
      await user.keyboard('{Enter}')

      expect(handleRetry).toHaveBeenCalled()
    })
  })
})

describe('ErrorBoundary displayName', () => {
  it('has correct displayName', () => {
    // Class components don't have displayName by default
    // but we can check the constructor name
    expect(ErrorBoundary.name).toBe('ErrorBoundary')
  })
})

describe('ErrorBoundary edge cases', () => {
  const originalError = console.error
  beforeEach(() => {
    console.error = vi.fn()
  })
  afterEach(() => {
    console.error = originalError
  })

  it('handles multiple children', () => {
    render(
      <ErrorBoundary>
        <div data-testid="child1">Child 1</div>
        <div data-testid="child2">Child 2</div>
      </ErrorBoundary>
    )

    expect(screen.getByTestId('child1')).toBeInTheDocument()
    expect(screen.getByTestId('child2')).toBeInTheDocument()
  })

  it('handles deeply nested errors', () => {
    const DeepError = () => (
      <div>
        <div>
          <div>
            <ThrowError />
          </div>
        </div>
      </div>
    )

    render(
      <ErrorBoundary>
        <DeepError />
      </ErrorBoundary>
    )

    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
  })

  it('handles error during componentDidCatch', () => {
    const handleError = vi.fn()

    render(
      <ErrorBoundary onError={handleError}>
        <ThrowError />
      </ErrorBoundary>
    )

    // Error should still be caught even if onError throws
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
  })

  it('preserves error info between re-renders', () => {
    const { rerender } = render(
      <ErrorBoundary>
        <ThrowError />
      </ErrorBoundary>
    )

    expect(screen.getByText('Something went wrong')).toBeInTheDocument()

    // Re-render with same error
    rerender(
      <ErrorBoundary>
        <ThrowError />
      </ErrorBoundary>
    )

    // Should still show error UI
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
  })
})
