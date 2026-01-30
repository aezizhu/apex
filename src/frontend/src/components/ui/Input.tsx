import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const inputVariants = cva(
  'flex w-full rounded-lg text-sm text-apex-text-primary placeholder:text-apex-text-muted transition-all focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
  {
    variants: {
      variant: {
        default:
          'bg-apex-bg-tertiary border border-apex-border-subtle focus:border-apex-accent-primary focus:ring-1 focus:ring-apex-accent-primary',
        ghost:
          'bg-transparent border-transparent hover:bg-apex-bg-tertiary focus:bg-apex-bg-tertiary focus:border-apex-border-default',
        filled:
          'bg-apex-bg-elevated border border-transparent focus:border-apex-accent-primary focus:ring-1 focus:ring-apex-accent-primary',
      },
      inputSize: {
        sm: 'h-8 px-3 text-xs',
        md: 'h-10 px-3 text-sm',
        lg: 'h-12 px-4 text-base',
      },
    },
    defaultVariants: {
      variant: 'default',
      inputSize: 'md',
    },
  }
)

export interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'size'>,
    VariantProps<typeof inputVariants> {
  error?: boolean
  errorMessage?: string
  label?: string
  hint?: string
  leftElement?: React.ReactNode
  rightElement?: React.ReactNode
}

const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      className,
      variant,
      inputSize,
      error,
      errorMessage,
      label,
      hint,
      leftElement,
      rightElement,
      type,
      id,
      ...props
    },
    ref
  ) => {
    const inputId = id || props.name

    return (
      <div className="w-full space-y-1.5">
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-apex-text-secondary"
          >
            {label}
          </label>
        )}
        <div className="relative">
          {leftElement && (
            <div className="absolute left-3 top-1/2 -translate-y-1/2 text-apex-text-tertiary">
              {leftElement}
            </div>
          )}
          <input
            type={type}
            id={inputId}
            className={cn(
              inputVariants({ variant, inputSize }),
              error && 'border-red-500 focus:border-red-500 focus:ring-red-500',
              leftElement && 'pl-10',
              rightElement && 'pr-10',
              className
            )}
            ref={ref}
            {...props}
          />
          {rightElement && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2 text-apex-text-tertiary">
              {rightElement}
            </div>
          )}
        </div>
        {(errorMessage || hint) && (
          <p
            className={cn(
              'text-xs',
              error ? 'text-red-500' : 'text-apex-text-tertiary'
            )}
          >
            {errorMessage || hint}
          </p>
        )}
      </div>
    )
  }
)

Input.displayName = 'Input'

// Textarea Component
const textareaVariants = cva(
  'flex w-full rounded-lg text-sm text-apex-text-primary placeholder:text-apex-text-muted transition-all focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 resize-none',
  {
    variants: {
      variant: {
        default:
          'bg-apex-bg-tertiary border border-apex-border-subtle focus:border-apex-accent-primary focus:ring-1 focus:ring-apex-accent-primary',
        ghost:
          'bg-transparent border-transparent hover:bg-apex-bg-tertiary focus:bg-apex-bg-tertiary focus:border-apex-border-default',
        filled:
          'bg-apex-bg-elevated border border-transparent focus:border-apex-accent-primary focus:ring-1 focus:ring-apex-accent-primary',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  }
)

export interface TextareaProps
  extends TextareaHTMLAttributes<HTMLTextAreaElement>,
    VariantProps<typeof textareaVariants> {
  error?: boolean
  errorMessage?: string
  label?: string
  hint?: string
}

const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(
  (
    { className, variant, error, errorMessage, label, hint, id, rows = 4, ...props },
    ref
  ) => {
    const textareaId = id || props.name

    return (
      <div className="w-full space-y-1.5">
        {label && (
          <label
            htmlFor={textareaId}
            className="block text-sm font-medium text-apex-text-secondary"
          >
            {label}
          </label>
        )}
        <textarea
          id={textareaId}
          rows={rows}
          className={cn(
            textareaVariants({ variant }),
            'px-3 py-2',
            error && 'border-red-500 focus:border-red-500 focus:ring-red-500',
            className
          )}
          ref={ref}
          {...props}
        />
        {(errorMessage || hint) && (
          <p
            className={cn(
              'text-xs',
              error ? 'text-red-500' : 'text-apex-text-tertiary'
            )}
          >
            {errorMessage || hint}
          </p>
        )}
      </div>
    )
  }
)

Textarea.displayName = 'Textarea'

// Search Input (specialized variant)
interface SearchInputProps extends Omit<InputProps, 'leftElement'> {
  onClear?: () => void
}

const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(
  ({ className, onClear, value, ...props }, ref) => {
    return (
      <Input
        ref={ref}
        type="search"
        className={className}
        value={value}
        leftElement={
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-4 w-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
        }
        rightElement={
          value && onClear ? (
            <button
              type="button"
              onClick={onClear}
              className="hover:text-apex-text-primary"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-4 w-4"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          ) : undefined
        }
        {...props}
      />
    )
  }
)

SearchInput.displayName = 'SearchInput'

export { Input, Textarea, SearchInput, inputVariants, textareaVariants }
