import { forwardRef, type HTMLAttributes } from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const cardVariants = cva('rounded-xl transition-all', {
  variants: {
    variant: {
      default: 'bg-apex-bg-secondary border border-apex-border-subtle',
      elevated: 'bg-apex-bg-elevated border border-apex-border-default shadow-lg',
      ghost: 'bg-transparent',
      glass:
        'bg-apex-bg-secondary/80 backdrop-blur-sm border border-apex-border-subtle',
      glow: 'bg-apex-bg-secondary border border-apex-accent-primary/30 shadow-glow-sm',
    },
    padding: {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
      xl: 'p-8',
    },
    interactive: {
      true: 'cursor-pointer hover:border-apex-border-strong hover:bg-apex-bg-tertiary',
      false: '',
    },
  },
  defaultVariants: {
    variant: 'default',
    padding: 'md',
    interactive: false,
  },
})

export interface CardProps
  extends HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof cardVariants> {}

const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, variant, padding, interactive, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(cardVariants({ variant, padding, interactive, className }))}
      {...props}
    />
  )
)

Card.displayName = 'Card'

// Card Header
interface CardHeaderProps extends HTMLAttributes<HTMLDivElement> {
  title?: string
  description?: string
  action?: React.ReactNode
}

const CardHeader = forwardRef<HTMLDivElement, CardHeaderProps>(
  ({ className, title, description, action, children, ...props }, ref) => (
    <div
      ref={ref}
      className={cn('flex items-start justify-between gap-4', className)}
      {...props}
    >
      {(title || description) ? (
        <div className="space-y-1">
          {title && (
            <h3 className="text-lg font-semibold text-apex-text-primary">{title}</h3>
          )}
          {description && (
            <p className="text-sm text-apex-text-secondary">{description}</p>
          )}
        </div>
      ) : (
        children
      )}
      {action && <div className="flex-shrink-0">{action}</div>}
    </div>
  )
)

CardHeader.displayName = 'CardHeader'

// Card Title
const CardTitle = forwardRef<HTMLHeadingElement, HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3
      ref={ref}
      className={cn('text-lg font-semibold text-apex-text-primary', className)}
      {...props}
    />
  )
)

CardTitle.displayName = 'CardTitle'

// Card Description
const CardDescription = forwardRef<
  HTMLParagraphElement,
  HTMLAttributes<HTMLParagraphElement>
>(({ className, ...props }, ref) => (
  <p
    ref={ref}
    className={cn('text-sm text-apex-text-secondary', className)}
    {...props}
  />
))

CardDescription.displayName = 'CardDescription'

// Card Content
const CardContent = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn('mt-4', className)} {...props} />
  )
)

CardContent.displayName = 'CardContent'

// Card Footer
const CardFooter = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn('mt-4 flex items-center gap-3 pt-4 border-t border-apex-border-subtle', className)}
      {...props}
    />
  )
)

CardFooter.displayName = 'CardFooter'

// Stat Card (specialized variant)
interface StatCardProps extends CardProps {
  label: string
  value: string | number
  change?: {
    value: number
    trend: 'up' | 'down' | 'neutral'
  }
  icon?: React.ReactNode
}

const StatCard = forwardRef<HTMLDivElement, StatCardProps>(
  ({ className, label, value, change, icon, ...props }, ref) => (
    <Card ref={ref} className={cn('', className)} {...props}>
      <div className="flex items-start justify-between">
        <div className="space-y-2">
          <p className="text-sm font-medium text-apex-text-secondary">{label}</p>
          <p className="text-2xl font-bold text-apex-text-primary">{value}</p>
          {change && (
            <p
              className={cn(
                'text-xs font-medium',
                change.trend === 'up' && 'text-green-500',
                change.trend === 'down' && 'text-red-500',
                change.trend === 'neutral' && 'text-apex-text-tertiary'
              )}
            >
              {change.trend === 'up' && '+'}
              {change.value}%
            </p>
          )}
        </div>
        {icon && (
          <div className="p-2 rounded-lg bg-apex-bg-tertiary text-apex-text-secondary">
            {icon}
          </div>
        )}
      </div>
    </Card>
  )
)

StatCard.displayName = 'StatCard'

export { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter, StatCard, cardVariants }
