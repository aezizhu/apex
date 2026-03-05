import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
  StatCard,
  cardVariants,
} from '@/components/ui/Card'

describe('Card', () => {
  describe('rendering', () => {
    it('renders with default props', () => {
      render(<Card data-testid="card">Content</Card>)
      const card = screen.getByTestId('card')
      expect(card).toBeInTheDocument()
      expect(card).toHaveTextContent('Content')
    })

    it('renders children correctly', () => {
      render(
        <Card>
          <span data-testid="child">Child Content</span>
        </Card>
      )
      expect(screen.getByTestId('child')).toBeInTheDocument()
    })

    it('forwards ref correctly', () => {
      const ref = vi.fn()
      render(<Card ref={ref}>Card</Card>)
      expect(ref).toHaveBeenCalled()
    })

    it('applies custom className', () => {
      render(
        <Card className="custom-class" data-testid="card">
          Card
        </Card>
      )
      expect(screen.getByTestId('card')).toHaveClass('custom-class')
    })

    it('sets displayName correctly', () => {
      expect(Card.displayName).toBe('Card')
    })
  })

  describe('variants', () => {
    it('renders default variant', () => {
      render(
        <Card variant="default" data-testid="card">
          Default
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('bg-apex-bg-secondary')
      expect(card).toHaveClass('border-apex-border-subtle')
    })

    it('renders elevated variant', () => {
      render(
        <Card variant="elevated" data-testid="card">
          Elevated
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('bg-apex-bg-elevated')
      expect(card).toHaveClass('shadow-lg')
    })

    it('renders ghost variant', () => {
      render(
        <Card variant="ghost" data-testid="card">
          Ghost
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('bg-transparent')
    })

    it('renders glass variant', () => {
      render(
        <Card variant="glass" data-testid="card">
          Glass
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('backdrop-blur-sm')
    })

    it('renders glow variant', () => {
      render(
        <Card variant="glow" data-testid="card">
          Glow
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('shadow-glow-sm')
    })
  })

  describe('padding', () => {
    it('renders with no padding', () => {
      render(
        <Card padding="none" data-testid="card">
          None
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).not.toHaveClass('p-3')
      expect(card).not.toHaveClass('p-4')
      expect(card).not.toHaveClass('p-6')
      expect(card).not.toHaveClass('p-8')
    })

    it('renders with sm padding', () => {
      render(
        <Card padding="sm" data-testid="card">
          SM
        </Card>
      )
      expect(screen.getByTestId('card')).toHaveClass('p-3')
    })

    it('renders with md padding (default)', () => {
      render(
        <Card padding="md" data-testid="card">
          MD
        </Card>
      )
      expect(screen.getByTestId('card')).toHaveClass('p-4')
    })

    it('renders with lg padding', () => {
      render(
        <Card padding="lg" data-testid="card">
          LG
        </Card>
      )
      expect(screen.getByTestId('card')).toHaveClass('p-6')
    })

    it('renders with xl padding', () => {
      render(
        <Card padding="xl" data-testid="card">
          XL
        </Card>
      )
      expect(screen.getByTestId('card')).toHaveClass('p-8')
    })
  })

  describe('interactive', () => {
    it('renders non-interactive by default', () => {
      render(<Card data-testid="card">Card</Card>)
      const card = screen.getByTestId('card')
      expect(card).not.toHaveClass('cursor-pointer')
    })

    it('renders interactive card with hover styles', () => {
      render(
        <Card interactive data-testid="card">
          Interactive
        </Card>
      )
      const card = screen.getByTestId('card')
      expect(card).toHaveClass('cursor-pointer')
      expect(card).toHaveClass('hover:border-apex-border-strong')
      expect(card).toHaveClass('hover:bg-apex-bg-tertiary')
    })

    it('handles click on interactive card', async () => {
      const handleClick = vi.fn()
      const user = userEvent.setup()
      render(
        <Card interactive onClick={handleClick} data-testid="card">
          Clickable
        </Card>
      )

      await user.click(screen.getByTestId('card'))
      expect(handleClick).toHaveBeenCalledTimes(1)
    })
  })
})

describe('CardHeader', () => {
  it('renders with title and description', () => {
    render(
      <CardHeader title="Test Title" description="Test Description" />
    )
    expect(screen.getByText('Test Title')).toBeInTheDocument()
    expect(screen.getByText('Test Description')).toBeInTheDocument()
  })

  it('renders children when no title/description', () => {
    render(
      <CardHeader>
        <span data-testid="custom-content">Custom Content</span>
      </CardHeader>
    )
    expect(screen.getByTestId('custom-content')).toBeInTheDocument()
  })

  it('renders action element', () => {
    render(
      <CardHeader
        title="Title"
        action={<button data-testid="action-btn">Action</button>}
      />
    )
    expect(screen.getByTestId('action-btn')).toBeInTheDocument()
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<CardHeader ref={ref} title="Title" />)
    expect(ref).toHaveBeenCalled()
  })

  it('applies custom className', () => {
    render(
      <CardHeader
        title="Title"
        className="custom-header"
        data-testid="header"
      />
    )
    expect(screen.getByTestId('header')).toHaveClass('custom-header')
  })

  it('sets displayName correctly', () => {
    expect(CardHeader.displayName).toBe('CardHeader')
  })
})

describe('CardTitle', () => {
  it('renders title text', () => {
    render(<CardTitle>Card Title</CardTitle>)
    expect(screen.getByText('Card Title')).toBeInTheDocument()
  })

  it('renders as h3 element', () => {
    render(<CardTitle>Title</CardTitle>)
    expect(screen.getByRole('heading', { level: 3 })).toBeInTheDocument()
  })

  it('applies custom className', () => {
    render(
      <CardTitle className="custom-title" data-testid="title">
        Title
      </CardTitle>
    )
    expect(screen.getByTestId('title')).toHaveClass('custom-title')
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<CardTitle ref={ref}>Title</CardTitle>)
    expect(ref).toHaveBeenCalled()
  })

  it('sets displayName correctly', () => {
    expect(CardTitle.displayName).toBe('CardTitle')
  })
})

describe('CardDescription', () => {
  it('renders description text', () => {
    render(<CardDescription>Description text</CardDescription>)
    expect(screen.getByText('Description text')).toBeInTheDocument()
  })

  it('applies secondary text styling', () => {
    render(
      <CardDescription data-testid="desc">Description</CardDescription>
    )
    expect(screen.getByTestId('desc')).toHaveClass('text-apex-text-secondary')
  })

  it('applies custom className', () => {
    render(
      <CardDescription className="custom-desc" data-testid="desc">
        Desc
      </CardDescription>
    )
    expect(screen.getByTestId('desc')).toHaveClass('custom-desc')
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<CardDescription ref={ref}>Desc</CardDescription>)
    expect(ref).toHaveBeenCalled()
  })

  it('sets displayName correctly', () => {
    expect(CardDescription.displayName).toBe('CardDescription')
  })
})

describe('CardContent', () => {
  it('renders content', () => {
    render(
      <CardContent>
        <p>Card content here</p>
      </CardContent>
    )
    expect(screen.getByText('Card content here')).toBeInTheDocument()
  })

  it('applies margin-top styling', () => {
    render(<CardContent data-testid="content">Content</CardContent>)
    expect(screen.getByTestId('content')).toHaveClass('mt-4')
  })

  it('applies custom className', () => {
    render(
      <CardContent className="custom-content" data-testid="content">
        Content
      </CardContent>
    )
    expect(screen.getByTestId('content')).toHaveClass('custom-content')
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<CardContent ref={ref}>Content</CardContent>)
    expect(ref).toHaveBeenCalled()
  })

  it('sets displayName correctly', () => {
    expect(CardContent.displayName).toBe('CardContent')
  })
})

describe('CardFooter', () => {
  it('renders footer content', () => {
    render(
      <CardFooter>
        <button>Cancel</button>
        <button>Save</button>
      </CardFooter>
    )
    expect(screen.getByText('Cancel')).toBeInTheDocument()
    expect(screen.getByText('Save')).toBeInTheDocument()
  })

  it('applies border and flex styling', () => {
    render(<CardFooter data-testid="footer">Footer</CardFooter>)
    const footer = screen.getByTestId('footer')
    expect(footer).toHaveClass('border-t')
    expect(footer).toHaveClass('flex')
    expect(footer).toHaveClass('items-center')
  })

  it('applies custom className', () => {
    render(
      <CardFooter className="custom-footer" data-testid="footer">
        Footer
      </CardFooter>
    )
    expect(screen.getByTestId('footer')).toHaveClass('custom-footer')
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<CardFooter ref={ref}>Footer</CardFooter>)
    expect(ref).toHaveBeenCalled()
  })

  it('sets displayName correctly', () => {
    expect(CardFooter.displayName).toBe('CardFooter')
  })
})

describe('StatCard', () => {
  it('renders label and value', () => {
    render(<StatCard label="Total Users" value={1234} />)
    expect(screen.getByText('Total Users')).toBeInTheDocument()
    expect(screen.getByText('1234')).toBeInTheDocument()
  })

  it('renders string value', () => {
    render(<StatCard label="Status" value="Active" />)
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('renders with upward change trend', () => {
    render(
      <StatCard
        label="Growth"
        value={100}
        change={{ value: 15, trend: 'up' }}
      />
    )
    const change = screen.getByText('+15%')
    expect(change).toBeInTheDocument()
    expect(change).toHaveClass('text-green-500')
  })

  it('renders with downward change trend', () => {
    render(
      <StatCard
        label="Decline"
        value={50}
        change={{ value: 10, trend: 'down' }}
      />
    )
    const change = screen.getByText('10%')
    expect(change).toBeInTheDocument()
    expect(change).toHaveClass('text-red-500')
  })

  it('renders with neutral change trend', () => {
    render(
      <StatCard
        label="Stable"
        value={100}
        change={{ value: 0, trend: 'neutral' }}
      />
    )
    const change = screen.getByText('0%')
    expect(change).toBeInTheDocument()
    expect(change).toHaveClass('text-apex-text-tertiary')
  })

  it('renders icon when provided', () => {
    render(
      <StatCard
        label="Users"
        value={100}
        icon={<span data-testid="stat-icon">Icon</span>}
      />
    )
    expect(screen.getByTestId('stat-icon')).toBeInTheDocument()
  })

  it('does not render change when not provided', () => {
    const { container } = render(<StatCard label="Simple" value={42} />)
    expect(container.querySelector('.text-green-500')).not.toBeInTheDocument()
    expect(container.querySelector('.text-red-500')).not.toBeInTheDocument()
  })

  it('applies Card props', () => {
    render(
      <StatCard
        label="Test"
        value={100}
        variant="elevated"
        data-testid="stat-card"
      />
    )
    expect(screen.getByTestId('stat-card')).toBeInTheDocument()
  })

  it('forwards ref correctly', () => {
    const ref = vi.fn()
    render(<StatCard ref={ref} label="Test" value={100} />)
    expect(ref).toHaveBeenCalled()
  })

  it('sets displayName correctly', () => {
    expect(StatCard.displayName).toBe('StatCard')
  })
})

describe('cardVariants', () => {
  it('generates correct classes for default variant with md padding', () => {
    const classes = cardVariants({ variant: 'default', padding: 'md' })
    expect(classes).toContain('bg-apex-bg-secondary')
    expect(classes).toContain('p-4')
  })

  it('generates correct classes for elevated variant', () => {
    const classes = cardVariants({ variant: 'elevated' })
    expect(classes).toContain('bg-apex-bg-elevated')
    expect(classes).toContain('shadow-lg')
  })

  it('generates correct classes for interactive', () => {
    const classes = cardVariants({ interactive: true })
    expect(classes).toContain('cursor-pointer')
    expect(classes).toContain('hover:border-apex-border-strong')
  })

  it('handles undefined values with defaults', () => {
    const classes = cardVariants({})
    expect(classes).toContain('bg-apex-bg-secondary') // default variant
    expect(classes).toContain('p-4') // default md padding
  })
})

describe('Card composition', () => {
  it('renders full card composition', () => {
    render(
      <Card variant="elevated" data-testid="full-card">
        <CardHeader
          title="Dashboard"
          description="Overview of your data"
          action={<button>Edit</button>}
        />
        <CardContent>
          <p>Main content goes here</p>
        </CardContent>
        <CardFooter>
          <button>Cancel</button>
          <button>Save</button>
        </CardFooter>
      </Card>
    )

    expect(screen.getByTestId('full-card')).toBeInTheDocument()
    expect(screen.getByText('Dashboard')).toBeInTheDocument()
    expect(screen.getByText('Overview of your data')).toBeInTheDocument()
    expect(screen.getByText('Edit')).toBeInTheDocument()
    expect(screen.getByText('Main content goes here')).toBeInTheDocument()
    expect(screen.getByText('Cancel')).toBeInTheDocument()
    expect(screen.getByText('Save')).toBeInTheDocument()
  })

  it('renders card with CardTitle and CardDescription', () => {
    render(
      <Card>
        <CardHeader>
          <div>
            <CardTitle>Custom Title</CardTitle>
            <CardDescription>Custom Description</CardDescription>
          </div>
        </CardHeader>
        <CardContent>Content</CardContent>
      </Card>
    )

    expect(screen.getByText('Custom Title')).toBeInTheDocument()
    expect(screen.getByText('Custom Description')).toBeInTheDocument()
    expect(screen.getByText('Content')).toBeInTheDocument()
  })
})
