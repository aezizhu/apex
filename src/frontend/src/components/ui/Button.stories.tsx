import type { Meta, StoryObj } from '@storybook/react'
import { fn } from '@storybook/test'
import { Button } from './Button'
import { ArrowRight, Download, Plus, Trash2, Settings, Search, Mail } from 'lucide-react'

const meta: Meta<typeof Button> = {
  title: 'UI/Button',
  component: Button,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component:
          'A versatile button component with multiple variants, sizes, and states. Supports loading states, icons, and accessibility features.',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['primary', 'secondary', 'ghost', 'danger', 'success', 'outline', 'link'],
      description: 'The visual style variant of the button',
    },
    size: {
      control: 'select',
      options: ['xs', 'sm', 'md', 'lg', 'xl', 'icon', 'icon-sm', 'icon-xs'],
      description: 'The size of the button',
    },
    loading: {
      control: 'boolean',
      description: 'Shows a loading spinner and disables the button',
    },
    disabled: {
      control: 'boolean',
      description: 'Disables the button',
    },
  },
  args: {
    onClick: fn(),
  },
}

export default meta
type Story = StoryObj<typeof meta>

// ============================================================================
// Basic Variants
// ============================================================================

export const Primary: Story = {
  args: {
    variant: 'primary',
    children: 'Primary Button',
  },
}

export const Secondary: Story = {
  args: {
    variant: 'secondary',
    children: 'Secondary Button',
  },
}

export const Ghost: Story = {
  args: {
    variant: 'ghost',
    children: 'Ghost Button',
  },
}

export const Danger: Story = {
  args: {
    variant: 'danger',
    children: 'Danger Button',
  },
}

export const Success: Story = {
  args: {
    variant: 'success',
    children: 'Success Button',
  },
}

export const Outline: Story = {
  args: {
    variant: 'outline',
    children: 'Outline Button',
  },
}

export const Link: Story = {
  args: {
    variant: 'link',
    children: 'Link Button',
  },
}

// ============================================================================
// All Variants Overview
// ============================================================================

export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4 items-center">
      <Button variant="primary">Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="ghost">Ghost</Button>
      <Button variant="danger">Danger</Button>
      <Button variant="success">Success</Button>
      <Button variant="outline">Outline</Button>
      <Button variant="link">Link</Button>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'All available button variants displayed together for comparison.',
      },
    },
  },
}

// ============================================================================
// Sizes
// ============================================================================

export const ExtraSmall: Story = {
  args: {
    size: 'xs',
    children: 'Extra Small',
  },
}

export const Small: Story = {
  args: {
    size: 'sm',
    children: 'Small Button',
  },
}

export const Medium: Story = {
  args: {
    size: 'md',
    children: 'Medium Button',
  },
}

export const Large: Story = {
  args: {
    size: 'lg',
    children: 'Large Button',
  },
}

export const ExtraLarge: Story = {
  args: {
    size: 'xl',
    children: 'Extra Large',
  },
}

export const AllSizes: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4 items-center">
      <Button size="xs">Extra Small</Button>
      <Button size="sm">Small</Button>
      <Button size="md">Medium</Button>
      <Button size="lg">Large</Button>
      <Button size="xl">Extra Large</Button>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'All available button sizes displayed together for comparison.',
      },
    },
  },
}

// ============================================================================
// Icon Buttons
// ============================================================================

export const IconButton: Story = {
  args: {
    size: 'icon',
    children: <Plus className="h-5 w-5" />,
    'aria-label': 'Add item',
  },
}

export const IconButtonSmall: Story = {
  args: {
    size: 'icon-sm',
    children: <Settings className="h-4 w-4" />,
    'aria-label': 'Settings',
  },
}

export const IconButtonExtraSmall: Story = {
  args: {
    size: 'icon-xs',
    variant: 'ghost',
    children: <Trash2 className="h-3 w-3" />,
    'aria-label': 'Delete',
  },
}

export const AllIconSizes: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4 items-center">
      <Button size="icon-xs" variant="ghost" aria-label="Extra small icon">
        <Search className="h-3 w-3" />
      </Button>
      <Button size="icon-sm" variant="secondary" aria-label="Small icon">
        <Settings className="h-4 w-4" />
      </Button>
      <Button size="icon" variant="primary" aria-label="Default icon">
        <Plus className="h-5 w-5" />
      </Button>
    </div>
  ),
}

// ============================================================================
// With Icons
// ============================================================================

export const WithLeftIcon: Story = {
  args: {
    children: 'Download',
    leftIcon: <Download className="h-4 w-4" />,
  },
}

export const WithRightIcon: Story = {
  args: {
    children: 'Continue',
    rightIcon: <ArrowRight className="h-4 w-4" />,
  },
}

export const WithBothIcons: Story = {
  args: {
    children: 'Send Email',
    leftIcon: <Mail className="h-4 w-4" />,
    rightIcon: <ArrowRight className="h-4 w-4" />,
  },
}

export const IconVariations: Story = {
  render: () => (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap gap-4">
        <Button leftIcon={<Plus className="h-4 w-4" />}>Add New</Button>
        <Button variant="secondary" leftIcon={<Download className="h-4 w-4" />}>
          Download
        </Button>
        <Button variant="danger" leftIcon={<Trash2 className="h-4 w-4" />}>
          Delete
        </Button>
      </div>
      <div className="flex flex-wrap gap-4">
        <Button rightIcon={<ArrowRight className="h-4 w-4" />}>Next Step</Button>
        <Button variant="success" rightIcon={<ArrowRight className="h-4 w-4" />}>
          Proceed
        </Button>
      </div>
    </div>
  ),
}

// ============================================================================
// States
// ============================================================================

export const Loading: Story = {
  args: {
    loading: true,
    children: 'Loading...',
  },
}

export const LoadingVariants: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4">
      <Button loading variant="primary">
        Saving...
      </Button>
      <Button loading variant="secondary">
        Processing...
      </Button>
      <Button loading variant="danger">
        Deleting...
      </Button>
      <Button loading variant="success">
        Completing...
      </Button>
    </div>
  ),
}

export const Disabled: Story = {
  args: {
    disabled: true,
    children: 'Disabled Button',
  },
}

export const DisabledVariants: Story = {
  render: () => (
    <div className="flex flex-wrap gap-4">
      <Button disabled variant="primary">
        Primary
      </Button>
      <Button disabled variant="secondary">
        Secondary
      </Button>
      <Button disabled variant="danger">
        Danger
      </Button>
      <Button disabled variant="outline">
        Outline
      </Button>
    </div>
  ),
}

// ============================================================================
// Interactive Examples
// ============================================================================

export const ButtonGroup: Story = {
  render: () => (
    <div className="flex">
      <Button variant="secondary" className="rounded-r-none border-r-0">
        Left
      </Button>
      <Button variant="secondary" className="rounded-none border-r-0">
        Center
      </Button>
      <Button variant="secondary" className="rounded-l-none">
        Right
      </Button>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'Buttons can be grouped together to form a button group.',
      },
    },
  },
}

export const FullWidth: Story = {
  render: () => (
    <div className="w-80 space-y-4">
      <Button className="w-full">Full Width Primary</Button>
      <Button variant="secondary" className="w-full">
        Full Width Secondary
      </Button>
      <Button variant="outline" className="w-full">
        Full Width Outline
      </Button>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'Buttons can be made full width using the w-full class.',
      },
    },
  },
}

export const ActionBar: Story = {
  render: () => (
    <div className="flex items-center justify-between w-96 p-4 bg-apex-bg-secondary rounded-lg border border-apex-border-subtle">
      <Button variant="ghost" size="sm">
        Cancel
      </Button>
      <div className="flex gap-2">
        <Button variant="secondary" size="sm">
          Save Draft
        </Button>
        <Button size="sm">Publish</Button>
      </div>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'Example of buttons used in an action bar pattern.',
      },
    },
  },
}

export const DangerZone: Story = {
  render: () => (
    <div className="w-96 p-4 bg-apex-bg-secondary rounded-lg border border-red-500/30">
      <h3 className="text-sm font-semibold text-red-400 mb-2">Danger Zone</h3>
      <p className="text-xs text-apex-text-secondary mb-4">
        This action cannot be undone. This will permanently delete the agent and all associated
        data.
      </p>
      <Button variant="danger" size="sm" leftIcon={<Trash2 className="h-4 w-4" />}>
        Delete Agent
      </Button>
    </div>
  ),
  parameters: {
    docs: {
      description: {
        story: 'Example of danger button used in a destructive action context.',
      },
    },
  },
}
