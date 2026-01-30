import { Component, type ErrorInfo, type ReactNode } from 'react'
import { AlertTriangle, RefreshCw, Home } from 'lucide-react'
import { Button } from '@/components/ui/Button'
import { Card } from '@/components/ui/Card'

interface ErrorBoundaryProps {
  children: ReactNode
  fallback?: ReactNode
  onError?: (error: Error, errorInfo: ErrorInfo) => void
  onReset?: () => void
}

interface ErrorBoundaryState {
  hasError: boolean
  error: Error | null
  errorInfo: ErrorInfo | null
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props)
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
    }
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return {
      hasError: true,
      error,
    }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.setState({ errorInfo })

    // Log to error reporting service
    console.error('Error caught by ErrorBoundary:', error, errorInfo)

    // Call optional error handler
    this.props.onError?.(error, errorInfo)
  }

  handleReset = (): void => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
    })
    this.props.onReset?.()
  }

  handleReload = (): void => {
    window.location.reload()
  }

  handleGoHome = (): void => {
    window.location.href = '/'
  }

  render(): ReactNode {
    if (this.state.hasError) {
      // Custom fallback if provided
      if (this.props.fallback) {
        return this.props.fallback
      }

      // Default error UI
      return (
        <div className="min-h-screen bg-apex-bg-primary flex items-center justify-center p-6">
          <Card variant="elevated" padding="lg" className="max-w-lg w-full">
            <div className="flex flex-col items-center text-center space-y-6">
              {/* Error Icon */}
              <div className="w-16 h-16 rounded-full bg-red-500/10 flex items-center justify-center">
                <AlertTriangle className="w-8 h-8 text-red-500" />
              </div>

              {/* Error Message */}
              <div className="space-y-2">
                <h2 className="text-xl font-semibold text-apex-text-primary">
                  Something went wrong
                </h2>
                <p className="text-sm text-apex-text-secondary">
                  An unexpected error occurred. Please try again or contact support if the
                  problem persists.
                </p>
              </div>

              {/* Error Details (collapsible in production) */}
              {import.meta.env.DEV && this.state.error && (
                <div className="w-full">
                  <details className="w-full">
                    <summary className="cursor-pointer text-sm text-apex-text-tertiary hover:text-apex-text-secondary">
                      View error details
                    </summary>
                    <div className="mt-3 p-3 bg-apex-bg-tertiary rounded-lg text-left overflow-auto max-h-48">
                      <p className="text-xs font-mono text-red-400 break-all">
                        {this.state.error.message}
                      </p>
                      {this.state.errorInfo?.componentStack && (
                        <pre className="mt-2 text-xxs font-mono text-apex-text-tertiary whitespace-pre-wrap">
                          {this.state.errorInfo.componentStack}
                        </pre>
                      )}
                    </div>
                  </details>
                </div>
              )}

              {/* Actions */}
              <div className="flex flex-col sm:flex-row gap-3 w-full sm:w-auto">
                <Button
                  variant="primary"
                  onClick={this.handleReset}
                  leftIcon={<RefreshCw className="w-4 h-4" />}
                  className="w-full sm:w-auto"
                >
                  Try Again
                </Button>
                <Button
                  variant="secondary"
                  onClick={this.handleGoHome}
                  leftIcon={<Home className="w-4 h-4" />}
                  className="w-full sm:w-auto"
                >
                  Go Home
                </Button>
              </div>
            </div>
          </Card>
        </div>
      )
    }

    return this.props.children
  }
}

// Hook-based error boundary wrapper for functional components
interface ErrorBoundaryWrapperProps {
  children: ReactNode
  fallback?: ReactNode
  onError?: (error: Error, errorInfo: ErrorInfo) => void
  resetKeys?: unknown[]
}

export function ErrorBoundaryWrapper({
  children,
  fallback,
  onError,
  resetKeys = [],
}: ErrorBoundaryWrapperProps): JSX.Element {
  // Generate a key from resetKeys to force remount on reset
  const key = resetKeys.map((k) => String(k)).join('-')

  return (
    <ErrorBoundary key={key} fallback={fallback} onError={onError}>
      {children}
    </ErrorBoundary>
  )
}

// Page-level error fallback component
interface PageErrorProps {
  title?: string
  message?: string
  onRetry?: () => void
  onGoBack?: () => void
}

export function PageError({
  title = 'Page Error',
  message = 'Failed to load this page. Please try again.',
  onRetry,
  onGoBack,
}: PageErrorProps): JSX.Element {
  return (
    <div className="flex flex-col items-center justify-center min-h-[400px] p-6 text-center">
      <AlertTriangle className="w-12 h-12 text-yellow-500 mb-4" />
      <h2 className="text-lg font-semibold text-apex-text-primary mb-2">{title}</h2>
      <p className="text-sm text-apex-text-secondary mb-6 max-w-md">{message}</p>
      <div className="flex gap-3">
        {onRetry && (
          <Button variant="primary" onClick={onRetry} size="sm">
            Retry
          </Button>
        )}
        {onGoBack && (
          <Button variant="secondary" onClick={onGoBack} size="sm">
            Go Back
          </Button>
        )}
      </div>
    </div>
  )
}

export default ErrorBoundary
