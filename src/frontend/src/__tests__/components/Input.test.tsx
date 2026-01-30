import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  Input,
  Textarea,
  SearchInput,
  inputVariants,
  textareaVariants,
} from '@/components/ui/Input'

describe('Input', () => {
  describe('rendering', () => {
    it('renders with default props', () => {
      render(<Input data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toBeInTheDocument()
      expect(input.tagName).toBe('INPUT')
    })

    it('renders with placeholder', () => {
      render(<Input placeholder="Enter text..." />)
      expect(screen.getByPlaceholderText('Enter text...')).toBeInTheDocument()
    })

    it('forwards ref correctly', () => {
      const ref = vi.fn()
      render(<Input ref={ref} />)
      expect(ref).toHaveBeenCalled()
    })

    it('applies custom className', () => {
      render(<Input className="custom-class" data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveClass('custom-class')
    })

    it('sets displayName correctly', () => {
      expect(Input.displayName).toBe('Input')
    })
  })

  describe('variants', () => {
    it('renders default variant', () => {
      render(<Input variant="default" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('bg-apex-bg-tertiary')
      expect(input).toHaveClass('border-apex-border-subtle')
    })

    it('renders ghost variant', () => {
      render(<Input variant="ghost" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('bg-transparent')
    })

    it('renders filled variant', () => {
      render(<Input variant="filled" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('bg-apex-bg-elevated')
    })
  })

  describe('sizes', () => {
    it('renders sm size', () => {
      render(<Input inputSize="sm" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('h-8')
      expect(input).toHaveClass('text-xs')
    })

    it('renders md size (default)', () => {
      render(<Input inputSize="md" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('h-10')
      expect(input).toHaveClass('text-sm')
    })

    it('renders lg size', () => {
      render(<Input inputSize="lg" data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('h-12')
      expect(input).toHaveClass('text-base')
    })
  })

  describe('label', () => {
    it('renders label when provided', () => {
      render(<Input label="Email Address" />)
      expect(screen.getByText('Email Address')).toBeInTheDocument()
    })

    it('associates label with input via htmlFor', () => {
      render(<Input label="Username" id="username-input" />)
      const label = screen.getByText('Username')
      expect(label).toHaveAttribute('for', 'username-input')
    })

    it('uses name as fallback id for label', () => {
      render(<Input label="Email" name="email" />)
      const label = screen.getByText('Email')
      expect(label).toHaveAttribute('for', 'email')
    })
  })

  describe('hint and error', () => {
    it('renders hint text', () => {
      render(<Input hint="Enter a valid email" />)
      expect(screen.getByText('Enter a valid email')).toBeInTheDocument()
    })

    it('renders error message when error is true', () => {
      render(<Input error errorMessage="This field is required" />)
      const errorText = screen.getByText('This field is required')
      expect(errorText).toBeInTheDocument()
      expect(errorText).toHaveClass('text-red-500')
    })

    it('applies error styling to input', () => {
      render(<Input error data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toHaveClass('border-red-500')
    })

    it('shows errorMessage over hint when both provided', () => {
      render(
        <Input
          hint="Helpful hint"
          error
          errorMessage="Error message"
        />
      )
      expect(screen.getByText('Error message')).toBeInTheDocument()
      expect(screen.queryByText('Helpful hint')).not.toBeInTheDocument()
    })
  })

  describe('left and right elements', () => {
    it('renders leftElement', () => {
      render(
        <Input leftElement={<span data-testid="left">$</span>} />
      )
      expect(screen.getByTestId('left')).toBeInTheDocument()
    })

    it('renders rightElement', () => {
      render(
        <Input rightElement={<span data-testid="right">.00</span>} />
      )
      expect(screen.getByTestId('right')).toBeInTheDocument()
    })

    it('applies padding when leftElement is present', () => {
      render(
        <Input
          leftElement={<span>$</span>}
          data-testid="input"
        />
      )
      expect(screen.getByTestId('input')).toHaveClass('pl-10')
    })

    it('applies padding when rightElement is present', () => {
      render(
        <Input
          rightElement={<span>.00</span>}
          data-testid="input"
        />
      )
      expect(screen.getByTestId('input')).toHaveClass('pr-10')
    })
  })

  describe('types', () => {
    it('renders text input by default', () => {
      render(<Input data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveAttribute('type', undefined)
    })

    it('renders password input', () => {
      render(<Input type="password" data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveAttribute('type', 'password')
    })

    it('renders email input', () => {
      render(<Input type="email" data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveAttribute('type', 'email')
    })

    it('renders number input', () => {
      render(<Input type="number" data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveAttribute('type', 'number')
    })
  })

  describe('states', () => {
    it('handles disabled state', () => {
      render(<Input disabled data-testid="input" />)
      const input = screen.getByTestId('input')
      expect(input).toBeDisabled()
      expect(input).toHaveClass('disabled:opacity-50')
    })

    it('handles readonly state', () => {
      render(<Input readOnly data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveAttribute('readonly')
    })

    it('handles required state', () => {
      render(<Input required data-testid="input" />)
      expect(screen.getByTestId('input')).toBeRequired()
    })
  })

  describe('interactions', () => {
    it('handles onChange events', async () => {
      const handleChange = vi.fn()
      const user = userEvent.setup()
      render(<Input onChange={handleChange} data-testid="input" />)

      await user.type(screen.getByTestId('input'), 'test')
      expect(handleChange).toHaveBeenCalled()
    })

    it('handles onFocus events', async () => {
      const handleFocus = vi.fn()
      const user = userEvent.setup()
      render(<Input onFocus={handleFocus} data-testid="input" />)

      await user.click(screen.getByTestId('input'))
      expect(handleFocus).toHaveBeenCalledTimes(1)
    })

    it('handles onBlur events', async () => {
      const handleBlur = vi.fn()
      const user = userEvent.setup()
      render(<Input onBlur={handleBlur} data-testid="input" />)

      const input = screen.getByTestId('input')
      await user.click(input)
      await user.tab()
      expect(handleBlur).toHaveBeenCalledTimes(1)
    })

    it('accepts value prop (controlled)', () => {
      render(<Input value="controlled value" onChange={() => {}} data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveValue('controlled value')
    })

    it('accepts defaultValue prop (uncontrolled)', () => {
      render(<Input defaultValue="default value" data-testid="input" />)
      expect(screen.getByTestId('input')).toHaveValue('default value')
    })
  })

  describe('accessibility', () => {
    it('has correct input role', () => {
      render(<Input />)
      expect(screen.getByRole('textbox')).toBeInTheDocument()
    })

    it('can have aria-label', () => {
      render(<Input aria-label="Search" />)
      expect(screen.getByRole('textbox', { name: 'Search' })).toBeInTheDocument()
    })

    it('associates label with input for screen readers', () => {
      render(<Input label="Email" id="email" />)
      const input = screen.getByLabelText('Email')
      expect(input).toBeInTheDocument()
    })

    it('can be focused via keyboard', async () => {
      const user = userEvent.setup()
      render(<Input data-testid="input" />)

      await user.tab()
      expect(screen.getByTestId('input')).toHaveFocus()
    })

    it('cannot be focused when disabled', async () => {
      const user = userEvent.setup()
      render(<Input disabled data-testid="input" />)

      await user.tab()
      expect(screen.getByTestId('input')).not.toHaveFocus()
    })
  })
})

describe('Textarea', () => {
  describe('rendering', () => {
    it('renders with default props', () => {
      render(<Textarea data-testid="textarea" />)
      const textarea = screen.getByTestId('textarea')
      expect(textarea).toBeInTheDocument()
      expect(textarea.tagName).toBe('TEXTAREA')
    })

    it('renders with placeholder', () => {
      render(<Textarea placeholder="Enter description..." />)
      expect(screen.getByPlaceholderText('Enter description...')).toBeInTheDocument()
    })

    it('forwards ref correctly', () => {
      const ref = vi.fn()
      render(<Textarea ref={ref} />)
      expect(ref).toHaveBeenCalled()
    })

    it('sets displayName correctly', () => {
      expect(Textarea.displayName).toBe('Textarea')
    })
  })

  describe('variants', () => {
    it('renders default variant', () => {
      render(<Textarea variant="default" data-testid="textarea" />)
      const textarea = screen.getByTestId('textarea')
      expect(textarea).toHaveClass('bg-apex-bg-tertiary')
    })

    it('renders ghost variant', () => {
      render(<Textarea variant="ghost" data-testid="textarea" />)
      const textarea = screen.getByTestId('textarea')
      expect(textarea).toHaveClass('bg-transparent')
    })

    it('renders filled variant', () => {
      render(<Textarea variant="filled" data-testid="textarea" />)
      const textarea = screen.getByTestId('textarea')
      expect(textarea).toHaveClass('bg-apex-bg-elevated')
    })
  })

  describe('rows', () => {
    it('uses default rows of 4', () => {
      render(<Textarea data-testid="textarea" />)
      expect(screen.getByTestId('textarea')).toHaveAttribute('rows', '4')
    })

    it('accepts custom rows', () => {
      render(<Textarea rows={10} data-testid="textarea" />)
      expect(screen.getByTestId('textarea')).toHaveAttribute('rows', '10')
    })
  })

  describe('label, hint, and error', () => {
    it('renders label', () => {
      render(<Textarea label="Description" />)
      expect(screen.getByText('Description')).toBeInTheDocument()
    })

    it('renders hint', () => {
      render(<Textarea hint="Max 500 characters" />)
      expect(screen.getByText('Max 500 characters')).toBeInTheDocument()
    })

    it('renders error message with styling', () => {
      render(<Textarea error errorMessage="Required field" />)
      const errorText = screen.getByText('Required field')
      expect(errorText).toHaveClass('text-red-500')
    })

    it('applies error border styling', () => {
      render(<Textarea error data-testid="textarea" />)
      expect(screen.getByTestId('textarea')).toHaveClass('border-red-500')
    })
  })

  describe('interactions', () => {
    it('handles onChange events', async () => {
      const handleChange = vi.fn()
      const user = userEvent.setup()
      render(<Textarea onChange={handleChange} data-testid="textarea" />)

      await user.type(screen.getByTestId('textarea'), 'test')
      expect(handleChange).toHaveBeenCalled()
    })

    it('handles value prop (controlled)', () => {
      render(<Textarea value="controlled" onChange={() => {}} data-testid="textarea" />)
      expect(screen.getByTestId('textarea')).toHaveValue('controlled')
    })
  })

  describe('accessibility', () => {
    it('has correct role', () => {
      render(<Textarea />)
      expect(screen.getByRole('textbox')).toBeInTheDocument()
    })

    it('associates label for screen readers', () => {
      render(<Textarea label="Notes" id="notes" />)
      expect(screen.getByLabelText('Notes')).toBeInTheDocument()
    })
  })
})

describe('SearchInput', () => {
  describe('rendering', () => {
    it('renders with search icon', () => {
      const { container } = render(<SearchInput data-testid="search" />)
      // Search icon should be rendered
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('renders as search type', () => {
      render(<SearchInput data-testid="search" />)
      expect(screen.getByTestId('search')).toHaveAttribute('type', 'search')
    })

    it('sets displayName correctly', () => {
      expect(SearchInput.displayName).toBe('SearchInput')
    })
  })

  describe('clear button', () => {
    it('shows clear button when value and onClear provided', () => {
      const handleClear = vi.fn()
      render(
        <SearchInput
          value="search text"
          onClear={handleClear}
          onChange={() => {}}
        />
      )
      // Clear button should be present (X icon)
      const clearButton = screen.getByRole('button')
      expect(clearButton).toBeInTheDocument()
    })

    it('does not show clear button when value is empty', () => {
      const handleClear = vi.fn()
      render(
        <SearchInput
          value=""
          onClear={handleClear}
          onChange={() => {}}
        />
      )
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })

    it('does not show clear button when onClear is not provided', () => {
      render(
        <SearchInput
          value="search text"
          onChange={() => {}}
        />
      )
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })

    it('calls onClear when clear button clicked', async () => {
      const handleClear = vi.fn()
      const user = userEvent.setup()
      render(
        <SearchInput
          value="search text"
          onClear={handleClear}
          onChange={() => {}}
        />
      )

      await user.click(screen.getByRole('button'))
      expect(handleClear).toHaveBeenCalledTimes(1)
    })
  })

  describe('interactions', () => {
    it('handles typing', async () => {
      const handleChange = vi.fn()
      const user = userEvent.setup()
      render(<SearchInput onChange={handleChange} data-testid="search" />)

      await user.type(screen.getByTestId('search'), 'query')
      expect(handleChange).toHaveBeenCalled()
    })

    it('forwards ref correctly', () => {
      const ref = vi.fn()
      render(<SearchInput ref={ref} />)
      expect(ref).toHaveBeenCalled()
    })
  })
})

describe('inputVariants', () => {
  it('generates correct classes for default variant md', () => {
    const classes = inputVariants({ variant: 'default', inputSize: 'md' })
    expect(classes).toContain('bg-apex-bg-tertiary')
    expect(classes).toContain('h-10')
  })

  it('generates correct classes for ghost variant sm', () => {
    const classes = inputVariants({ variant: 'ghost', inputSize: 'sm' })
    expect(classes).toContain('bg-transparent')
    expect(classes).toContain('h-8')
  })

  it('handles undefined values with defaults', () => {
    const classes = inputVariants({})
    expect(classes).toContain('bg-apex-bg-tertiary') // default variant
    expect(classes).toContain('h-10') // default md size
  })
})

describe('textareaVariants', () => {
  it('generates correct classes for default variant', () => {
    const classes = textareaVariants({ variant: 'default' })
    expect(classes).toContain('bg-apex-bg-tertiary')
  })

  it('generates correct classes for filled variant', () => {
    const classes = textareaVariants({ variant: 'filled' })
    expect(classes).toContain('bg-apex-bg-elevated')
  })

  it('handles undefined values with defaults', () => {
    const classes = textareaVariants({})
    expect(classes).toContain('bg-apex-bg-tertiary') // default variant
  })
})

describe('Input edge cases', () => {
  it('handles rapid typing', async () => {
    const handleChange = vi.fn()
    const user = userEvent.setup()
    render(<Input onChange={handleChange} data-testid="input" />)

    await user.type(screen.getByTestId('input'), 'quick typing test')
    expect(handleChange).toHaveBeenCalledTimes(17) // One for each character
  })

  it('handles special characters', async () => {
    const user = userEvent.setup()
    render(<Input data-testid="input" />)

    await user.type(screen.getByTestId('input'), '!@#$%^&*()')
    expect(screen.getByTestId('input')).toHaveValue('!@#$%^&*()')
  })

  it('handles paste events', async () => {
    const user = userEvent.setup()
    render(<Input data-testid="input" />)

    const input = screen.getByTestId('input')
    await user.click(input)
    await user.paste('pasted content')
    expect(input).toHaveValue('pasted content')
  })

  it('handles maxLength attribute', () => {
    render(<Input maxLength={10} data-testid="input" />)
    expect(screen.getByTestId('input')).toHaveAttribute('maxLength', '10')
  })

  it('handles minLength attribute', () => {
    render(<Input minLength={5} data-testid="input" />)
    expect(screen.getByTestId('input')).toHaveAttribute('minLength', '5')
  })

  it('handles pattern attribute', () => {
    render(<Input pattern="[0-9]*" data-testid="input" />)
    expect(screen.getByTestId('input')).toHaveAttribute('pattern', '[0-9]*')
  })

  it('handles autoComplete attribute', () => {
    render(<Input autoComplete="email" data-testid="input" />)
    expect(screen.getByTestId('input')).toHaveAttribute('autoComplete', 'email')
  })
})
