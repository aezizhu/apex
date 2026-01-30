import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Button, buttonVariants } from '@/components/ui/Button'

describe('Button', () => {
  describe('rendering', () => {
    it('renders with default props', () => {
      render(<Button>Click me</Button>)
      const button = screen.getByRole('button', { name: 'Click me' })
      expect(button).toBeInTheDocument()
      expect(button).toHaveClass('bg-apex-accent-primary') // primary variant
    })

    it('renders children correctly', () => {
      render(
        <Button>
          <span data-testid="child">Custom Child</span>
        </Button>
      )
      expect(screen.getByTestId('child')).toBeInTheDocument()
    })

    it('forwards ref correctly', () => {
      const ref = vi.fn()
      render(<Button ref={ref}>Button</Button>)
      expect(ref).toHaveBeenCalled()
    })

    it('applies custom className', () => {
      render(<Button className="custom-class">Button</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('custom-class')
    })

    it('sets displayName correctly', () => {
      expect(Button.displayName).toBe('Button')
    })
  })

  describe('variants', () => {
    it('renders primary variant', () => {
      render(<Button variant="primary">Primary</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('bg-apex-accent-primary')
    })

    it('renders secondary variant', () => {
      render(<Button variant="secondary">Secondary</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('bg-apex-bg-tertiary')
    })

    it('renders ghost variant', () => {
      render(<Button variant="ghost">Ghost</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('hover:bg-apex-bg-tertiary')
    })

    it('renders danger variant', () => {
      render(<Button variant="danger">Danger</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('bg-red-600')
    })

    it('renders success variant', () => {
      render(<Button variant="success">Success</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('bg-green-600')
    })

    it('renders outline variant', () => {
      render(<Button variant="outline">Outline</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('border-apex-border-default')
      expect(button).toHaveClass('bg-transparent')
    })

    it('renders link variant', () => {
      render(<Button variant="link">Link</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('text-apex-accent-primary')
      expect(button).toHaveClass('hover:underline')
    })
  })

  describe('sizes', () => {
    it('renders xs size', () => {
      render(<Button size="xs">XS</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-7')
      expect(button).toHaveClass('px-2')
    })

    it('renders sm size', () => {
      render(<Button size="sm">SM</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-8')
      expect(button).toHaveClass('px-3')
    })

    it('renders md size (default)', () => {
      render(<Button size="md">MD</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-10')
      expect(button).toHaveClass('px-4')
    })

    it('renders lg size', () => {
      render(<Button size="lg">LG</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-11')
      expect(button).toHaveClass('px-6')
    })

    it('renders xl size', () => {
      render(<Button size="xl">XL</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-12')
      expect(button).toHaveClass('px-8')
    })

    it('renders icon size', () => {
      render(<Button size="icon">I</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-10')
      expect(button).toHaveClass('w-10')
    })

    it('renders icon-sm size', () => {
      render(<Button size="icon-sm">I</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-8')
      expect(button).toHaveClass('w-8')
    })

    it('renders icon-xs size', () => {
      render(<Button size="icon-xs">I</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('h-6')
      expect(button).toHaveClass('w-6')
    })
  })

  describe('states', () => {
    it('handles disabled state', () => {
      render(<Button disabled>Disabled</Button>)
      const button = screen.getByRole('button')
      expect(button).toBeDisabled()
      expect(button).toHaveClass('disabled:opacity-50')
      expect(button).toHaveClass('disabled:pointer-events-none')
    })

    it('handles loading state', () => {
      render(<Button loading>Loading</Button>)
      const button = screen.getByRole('button')
      expect(button).toBeDisabled()
      // Should show loader icon (Loader2 with animate-spin)
      const loader = button.querySelector('.animate-spin')
      expect(loader).toBeInTheDocument()
    })

    it('disables button when loading', () => {
      render(<Button loading>Loading</Button>)
      const button = screen.getByRole('button')
      expect(button).toBeDisabled()
    })

    it('hides leftIcon when loading', () => {
      const LeftIcon = () => <span data-testid="left-icon">L</span>
      render(
        <Button loading leftIcon={<LeftIcon />}>
          Loading
        </Button>
      )
      expect(screen.queryByTestId('left-icon')).not.toBeInTheDocument()
    })

    it('hides rightIcon when loading', () => {
      const RightIcon = () => <span data-testid="right-icon">R</span>
      render(
        <Button loading rightIcon={<RightIcon />}>
          Loading
        </Button>
      )
      expect(screen.queryByTestId('right-icon')).not.toBeInTheDocument()
    })
  })

  describe('icons', () => {
    it('renders leftIcon', () => {
      const LeftIcon = () => <span data-testid="left-icon">L</span>
      render(<Button leftIcon={<LeftIcon />}>With Left Icon</Button>)
      expect(screen.getByTestId('left-icon')).toBeInTheDocument()
    })

    it('renders rightIcon', () => {
      const RightIcon = () => <span data-testid="right-icon">R</span>
      render(<Button rightIcon={<RightIcon />}>With Right Icon</Button>)
      expect(screen.getByTestId('right-icon')).toBeInTheDocument()
    })

    it('renders both icons', () => {
      const LeftIcon = () => <span data-testid="left-icon">L</span>
      const RightIcon = () => <span data-testid="right-icon">R</span>
      render(
        <Button leftIcon={<LeftIcon />} rightIcon={<RightIcon />}>
          Both Icons
        </Button>
      )
      expect(screen.getByTestId('left-icon')).toBeInTheDocument()
      expect(screen.getByTestId('right-icon')).toBeInTheDocument()
    })
  })

  describe('interactions', () => {
    it('handles click events', async () => {
      const handleClick = vi.fn()
      const user = userEvent.setup()
      render(<Button onClick={handleClick}>Click</Button>)

      await user.click(screen.getByRole('button'))
      expect(handleClick).toHaveBeenCalledTimes(1)
    })

    it('does not fire click when disabled', async () => {
      const handleClick = vi.fn()
      const user = userEvent.setup()
      render(
        <Button disabled onClick={handleClick}>
          Disabled
        </Button>
      )

      await user.click(screen.getByRole('button'))
      expect(handleClick).not.toHaveBeenCalled()
    })

    it('does not fire click when loading', async () => {
      const handleClick = vi.fn()
      const user = userEvent.setup()
      render(
        <Button loading onClick={handleClick}>
          Loading
        </Button>
      )

      await user.click(screen.getByRole('button'))
      expect(handleClick).not.toHaveBeenCalled()
    })

    it('handles keyboard events', () => {
      const handleKeyDown = vi.fn()
      render(<Button onKeyDown={handleKeyDown}>Button</Button>)

      fireEvent.keyDown(screen.getByRole('button'), { key: 'Enter' })
      expect(handleKeyDown).toHaveBeenCalledTimes(1)
    })
  })

  describe('accessibility', () => {
    it('has correct button role', () => {
      render(<Button>Accessible</Button>)
      expect(screen.getByRole('button')).toBeInTheDocument()
    })

    it('can have aria-label', () => {
      render(<Button aria-label="Close dialog">X</Button>)
      expect(screen.getByRole('button', { name: 'Close dialog' })).toBeInTheDocument()
    })

    it('has focus-visible styles', () => {
      render(<Button>Focusable</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveClass('focus-visible:outline-none')
      expect(button).toHaveClass('focus-visible:ring-2')
    })

    it('can be focused via keyboard', async () => {
      const user = userEvent.setup()
      render(<Button>Focusable</Button>)

      await user.tab()
      expect(screen.getByRole('button')).toHaveFocus()
    })

    it('cannot be focused when disabled', async () => {
      const user = userEvent.setup()
      render(<Button disabled>Disabled</Button>)

      await user.tab()
      expect(screen.getByRole('button')).not.toHaveFocus()
    })

    it('supports type attribute', () => {
      render(<Button type="submit">Submit</Button>)
      const button = screen.getByRole('button')
      expect(button).toHaveAttribute('type', 'submit')
    })
  })

  describe('buttonVariants', () => {
    it('generates correct classes for primary md', () => {
      const classes = buttonVariants({ variant: 'primary', size: 'md' })
      expect(classes).toContain('bg-apex-accent-primary')
      expect(classes).toContain('h-10')
    })

    it('generates correct classes for danger lg', () => {
      const classes = buttonVariants({ variant: 'danger', size: 'lg' })
      expect(classes).toContain('bg-red-600')
      expect(classes).toContain('h-11')
    })

    it('handles undefined values with defaults', () => {
      const classes = buttonVariants({})
      expect(classes).toContain('bg-apex-accent-primary') // default primary
      expect(classes).toContain('h-10') // default md
    })
  })

  describe('edge cases', () => {
    it('handles empty children', () => {
      render(<Button />)
      expect(screen.getByRole('button')).toBeInTheDocument()
    })

    it('handles null className', () => {
      render(<Button className={undefined}>Button</Button>)
      expect(screen.getByRole('button')).toBeInTheDocument()
    })

    it('passes through HTML button props', () => {
      render(
        <Button
          data-testid="custom-button"
          name="submit-btn"
          value="submit"
          form="my-form"
        >
          Button
        </Button>
      )
      const button = screen.getByTestId('custom-button')
      expect(button).toHaveAttribute('name', 'submit-btn')
      expect(button).toHaveAttribute('value', 'submit')
      expect(button).toHaveAttribute('form', 'my-form')
    })

    it('combines disabled and loading correctly', () => {
      render(
        <Button disabled loading>
          Both
        </Button>
      )
      const button = screen.getByRole('button')
      expect(button).toBeDisabled()
    })
  })
})
