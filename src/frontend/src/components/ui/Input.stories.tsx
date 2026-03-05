import type { Meta, StoryObj } from '@storybook/react'
import { fn } from '@storybook/test'
import { useState } from 'react'
import { Input, Textarea, SearchInput } from './Input'
import { Mail, Lock, Eye, EyeOff, User, AlertCircle, Check, DollarSign, Calendar } from 'lucide-react'
import { Button } from './Button'

const meta: Meta<typeof Input> = {
  title: 'UI/Input',
  component: Input,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component:
          'A comprehensive input component with multiple variants, sizes, validation states, and support for labels, hints, and icons.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'ghost', 'filled'],
      description: 'The visual style variant of the input',
    },
    inputSize: {
      control: 'select',
      options: ['sm', 'md', 'lg'],
      description: 'The size of the input',
    },
    error: {
      control: 'boolean',
      description: 'Shows error state',
    },
    disabled: {
      control: 'boolean',
      description: 'Disables the input',
    },
    label: {
      control: 'text',
      description: 'Label text displayed above the input',
    },
    hint: {
      control: 'text',
      description: 'Hint text displayed below the input',
    },
    errorMessage: {
      control: 'text',
      description: 'Error message displayed below the input when in error state',
    },
    placeholder: {
      control: 'text',
      description: 'Placeholder text',
    },
  },
  args: {
    onChange: fn(),
    onFocus: fn(),
    onBlur: fn(),
  },
}

export default meta
type Story = StoryObj<typeof meta>

// ============================================================================
// Basic Variants
// ============================================================================

export const Default: Story = {
  args: {
    variant: 'default',
    placeholder: 'Enter text...',
  },
}

export const Ghost: Story = {
  args: {
    variant: 'ghost',
    placeholder: 'Ghost input...',
  },
}

export const Filled: Story = {
  args: {
    variant: 'filled',
    placeholder: 'Filled input...',
  },
}

export const AllVariants: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Input variant="default" placeholder="Default variant" />
      <Input variant="ghost" placeholder="Ghost variant" />
      <Input variant="filled" placeholder="Filled variant" />
    </div>
  ),
}

// ============================================================================
// Sizes
// ============================================================================

export const Small: Story = {
  args: {
    inputSize: 'sm',
    placeholder: 'Small input',
  },
}

export const Medium: Story = {
  args: {
    inputSize: 'md',
    placeholder: 'Medium input',
  },
}

export const Large: Story = {
  args: {
    inputSize: 'lg',
    placeholder: 'Large input',
  },
}

export const AllSizes: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Input inputSize="sm" placeholder="Small size" />
      <Input inputSize="md" placeholder="Medium size (default)" />
      <Input inputSize="lg" placeholder="Large size" />
    </div>
  ),
}

// ============================================================================
// With Labels and Hints
// ============================================================================

export const WithLabel: Story = {
  args: {
    label: 'Email Address',
    placeholder: 'you@example.com',
    type: 'email',
  },
}

export const WithHint: Story = {
  args: {
    label: 'Password',
    placeholder: 'Enter password',
    type: 'password',
    hint: 'Must be at least 8 characters',
  },
}

export const WithLabelAndHint: Story = {
  args: {
    label: 'API Key',
    placeholder: 'sk-...',
    hint: 'Keep this key secret and never share it publicly',
  },
}

// ============================================================================
// With Icons
// ============================================================================

export const WithLeftIcon: Story = {
  args: {
    placeholder: 'Enter email',
    leftElement: <Mail className="h-4 w-4" />,
  },
}

export const WithRightIcon: Story = {
  args: {
    placeholder: 'Amount',
    rightElement: <DollarSign className="h-4 w-4" />,
  },
}

export const WithBothIcons: Story = {
  args: {
    placeholder: 'Search users...',
    leftElement: <User className="h-4 w-4" />,
    rightElement: <Check className="h-4 w-4 text-green-500" />,
  },
}

export const PasswordWithToggle: Story = {
  render: () => {
    const [showPassword, setShowPassword] = useState(false)
    return (
      <div className="w-80">
        <Input
          label="Password"
          type={showPassword ? 'text' : 'password'}
          placeholder="Enter password"
          leftElement={<Lock className="h-4 w-4" />}
          rightElement={
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="hover:text-apex-text-primary"
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          }
        />
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Password input with visibility toggle functionality.',
      },
    },
  },
}

export const IconVariations: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Input
        label="Email"
        placeholder="you@example.com"
        leftElement={<Mail className="h-4 w-4" />}
      />
      <Input
        label="Username"
        placeholder="@username"
        leftElement={<User className="h-4 w-4" />}
      />
      <Input
        label="Schedule"
        placeholder="Select date"
        leftElement={<Calendar className="h-4 w-4" />}
      />
    </div>
  ),
}

// ============================================================================
// States
// ============================================================================

export const Disabled: Story = {
  args: {
    label: 'Disabled Input',
    placeholder: 'Cannot edit',
    disabled: true,
    value: 'Disabled value',
  },
}

export const ReadOnly: Story = {
  args: {
    label: 'Read Only',
    value: 'This value cannot be changed',
    readOnly: true,
  },
}

export const ErrorState: Story = {
  args: {
    label: 'Email',
    placeholder: 'Enter email',
    error: true,
    errorMessage: 'Please enter a valid email address',
    value: 'invalid-email',
    leftElement: <Mail className="h-4 w-4" />,
  },
}

export const SuccessState: Story = {
  render: () => (
    <div className="w-80">
      <Input
        label="Email"
        placeholder="Enter email"
        value="valid@example.com"
        leftElement={<Mail className="h-4 w-4" />}
        rightElement={<Check className="h-4 w-4 text-green-500" />}
        hint="Email is valid"
        className="border-green-500 focus:border-green-500 focus:ring-green-500"
      />
    </div>
  ),
}

export const ValidationStates: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Input
        label="Valid Input"
        value="correct@email.com"
        rightElement={<Check className="h-4 w-4 text-green-500" />}
        hint="Looks good!"
      />
      <Input
        label="Invalid Input"
        value="wrong"
        error
        errorMessage="This field is required"
        rightElement={<AlertCircle className="h-4 w-4 text-red-500" />}
      />
      <Input
        label="Disabled Input"
        value="Disabled"
        disabled
        hint="This field is disabled"
      />
    </div>
  ),
}

// ============================================================================
// Input Types
// ============================================================================

export const TextInput: Story = {
  args: {
    label: 'Full Name',
    type: 'text',
    placeholder: 'John Doe',
  },
}

export const EmailInput: Story = {
  args: {
    label: 'Email',
    type: 'email',
    placeholder: 'you@example.com',
    leftElement: <Mail className="h-4 w-4" />,
  },
}

export const NumberInput: Story = {
  args: {
    label: 'Amount',
    type: 'number',
    placeholder: '0.00',
    leftElement: <DollarSign className="h-4 w-4" />,
  },
}

export const DateInput: Story = {
  args: {
    label: 'Date',
    type: 'date',
  },
}

export const AllInputTypes: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Input label="Text" type="text" placeholder="Enter text" />
      <Input label="Email" type="email" placeholder="email@example.com" />
      <Input label="Password" type="password" placeholder="********" />
      <Input label="Number" type="number" placeholder="0" />
      <Input label="Date" type="date" />
      <Input label="Time" type="time" />
      <Input label="URL" type="url" placeholder="https://example.com" />
    </div>
  ),
}

// ============================================================================
// Search Input
// ============================================================================

export const BasicSearchInput: Story = {
  render: () => {
    const [value, setValue] = useState('')
    return (
      <div className="w-80">
        <SearchInput
          placeholder="Search agents..."
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onClear={() => setValue('')}
        />
      </div>
    )
  },
}

export const SearchWithResults: Story = {
  render: () => {
    const [value, setValue] = useState('Agent')
    return (
      <div className="w-80 space-y-2">
        <SearchInput
          placeholder="Search agents..."
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onClear={() => setValue('')}
        />
        {value && (
          <div className="p-2 rounded-lg bg-apex-bg-tertiary border border-apex-border-subtle">
            <p className="text-xs text-apex-text-tertiary mb-2">Results for "{value}"</p>
            {['Agent Alpha', 'Agent Beta', 'Agent Gamma'].map((name) => (
              <div
                key={name}
                className="px-3 py-2 rounded hover:bg-apex-bg-elevated cursor-pointer text-sm"
              >
                {name}
              </div>
            ))}
          </div>
        )}
      </div>
    )
  },
}

// ============================================================================
// Textarea
// ============================================================================

export const BasicTextarea: Story = {
  render: () => (
    <div className="w-80">
      <Textarea placeholder="Enter your message..." rows={4} />
    </div>
  ),
}

export const TextareaWithLabel: Story = {
  render: () => (
    <div className="w-80">
      <Textarea
        label="Description"
        placeholder="Enter a detailed description..."
        hint="Max 500 characters"
        rows={4}
      />
    </div>
  ),
}

export const TextareaWithError: Story = {
  render: () => (
    <div className="w-80">
      <Textarea
        label="Bio"
        placeholder="Tell us about yourself..."
        error
        errorMessage="Bio must be at least 50 characters"
        value="Too short"
        rows={4}
      />
    </div>
  ),
}

export const TextareaVariants: Story = {
  render: () => (
    <div className="space-y-4 w-80">
      <Textarea variant="default" label="Default" placeholder="Default textarea" />
      <Textarea variant="ghost" label="Ghost" placeholder="Ghost textarea" />
      <Textarea variant="filled" label="Filled" placeholder="Filled textarea" />
    </div>
  ),
}

// ============================================================================
// Form Examples
// ============================================================================

export const LoginForm: Story = {
  render: () => (
    <div className="w-80 space-y-4 p-6 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle">
      <h2 className="text-lg font-semibold text-apex-text-primary">Sign In</h2>
      <Input
        label="Email"
        type="email"
        placeholder="you@example.com"
        leftElement={<Mail className="h-4 w-4" />}
      />
      <Input
        label="Password"
        type="password"
        placeholder="Enter password"
        leftElement={<Lock className="h-4 w-4" />}
      />
      <Button className="w-full">Sign In</Button>
      <p className="text-xs text-center text-apex-text-tertiary">
        Don't have an account?{' '}
        <a href="#" className="text-apex-accent-primary hover:underline">
          Sign up
        </a>
      </p>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A complete login form example using Input components.',
      },
    },
  },
}

export const AgentConfigForm: Story = {
  render: () => (
    <div className="w-96 space-y-4 p-6 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle">
      <h2 className="text-lg font-semibold text-apex-text-primary">Create New Agent</h2>
      <Input label="Agent Name" placeholder="e.g., Research Assistant" />
      <Input
        label="Model"
        placeholder="gpt-4-turbo"
        hint="Select the AI model for this agent"
      />
      <div className="grid grid-cols-2 gap-4">
        <Input
          label="Max Tokens"
          type="number"
          placeholder="4096"
        />
        <Input
          label="Temperature"
          type="number"
          placeholder="0.7"
        />
      </div>
      <Textarea
        label="System Prompt"
        placeholder="Define the agent's behavior and instructions..."
        rows={4}
      />
      <div className="flex gap-3 pt-2">
        <Button variant="secondary" className="flex-1">
          Cancel
        </Button>
        <Button className="flex-1">Create Agent</Button>
      </div>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'A configuration form for creating a new AI agent.',
      },
    },
  },
}

export const FilterForm: Story = {
  render: () => {
    const [search, setSearch] = useState('')
    return (
      <div className="w-80 space-y-4 p-4 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle">
        <h3 className="text-sm font-semibold text-apex-text-primary">Filter Agents</h3>
        <SearchInput
          placeholder="Search by name..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          onClear={() => setSearch('')}
          inputSize="sm"
        />
        <div className="grid grid-cols-2 gap-3">
          <Input label="Status" placeholder="All" inputSize="sm" />
          <Input label="Model" placeholder="Any" inputSize="sm" />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Input label="Min Success" type="number" placeholder="0%" inputSize="sm" />
          <Input label="Max Cost" type="number" placeholder="$100" inputSize="sm" />
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" className="flex-1">
            Reset
          </Button>
          <Button size="sm" className="flex-1">
            Apply
          </Button>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'A filter form with search and multiple filter inputs.',
      },
    },
  },
}
