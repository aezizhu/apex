import { useCallback } from 'react'
import {
  ListTodo,
  GitBranch,
  GitFork,
  GitMerge,
  ShieldCheck,
  Download,
  Upload,
  Trash2,
  ZoomIn,
  ZoomOut,
  Maximize,
  Undo2,
  Redo2,
  Play,
  Save,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/Button'
import { WorkflowNodeType, NODE_TYPE_CONFIG } from './types'

// ═══════════════════════════════════════════════════════════════════════════════
// Icon Map
// ═══════════════════════════════════════════════════════════════════════════════

const ICON_MAP: Record<WorkflowNodeType, React.ComponentType<{ size?: string | number; className?: string }>> = {
  task: ListTodo,
  condition: GitBranch,
  fork: GitFork,
  join: GitMerge,
  approval: ShieldCheck,
}

const NODE_PALETTE: WorkflowNodeType[] = ['task', 'condition', 'fork', 'join', 'approval']

// ═══════════════════════════════════════════════════════════════════════════════
// WorkflowToolbar Component
// ═══════════════════════════════════════════════════════════════════════════════

interface WorkflowToolbarProps {
  onAddNode: (type: WorkflowNodeType) => void
  onDelete: () => void
  onZoomIn: () => void
  onZoomOut: () => void
  onFitView: () => void
  onUndo: () => void
  onRedo: () => void
  onExport: () => void
  onImport: () => void
  onSave: () => void
  onExecute: () => void
  canUndo: boolean
  canRedo: boolean
  hasSelection: boolean
  workflowName: string
  onNameChange: (name: string) => void
}

export function WorkflowToolbar({
  onAddNode,
  onDelete,
  onZoomIn,
  onZoomOut,
  onFitView,
  onUndo,
  onRedo,
  onExport,
  onImport,
  onSave,
  onExecute,
  canUndo,
  canRedo,
  hasSelection,
  workflowName,
  onNameChange,
}: WorkflowToolbarProps) {
  const handleDragStart = useCallback(
    (type: WorkflowNodeType) => (e: React.DragEvent) => {
      e.dataTransfer.setData('workflow-node-type', type)
      e.dataTransfer.effectAllowed = 'copy'
    },
    []
  )

  return (
    <div className="flex flex-col gap-3">
      {/* Workflow name and actions */}
      <div className="flex items-center justify-between bg-apex-bg-secondary rounded-xl border border-apex-border-subtle px-4 py-3">
        <div className="flex items-center gap-3">
          <input
            type="text"
            value={workflowName}
            onChange={(e) => onNameChange(e.target.value)}
            className="bg-transparent text-lg font-semibold text-apex-text-primary border-none outline-none focus:ring-0 w-64 placeholder:text-apex-text-muted"
            placeholder="Untitled Workflow"
          />
        </div>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="icon-sm" onClick={onUndo} disabled={!canUndo} title="Undo">
            <Undo2 size={16} />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={onRedo} disabled={!canRedo} title="Redo">
            <Redo2 size={16} />
          </Button>

          <div className="w-px h-6 bg-apex-border-subtle mx-1" />

          <Button variant="ghost" size="icon-sm" onClick={onZoomIn} title="Zoom In">
            <ZoomIn size={16} />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={onZoomOut} title="Zoom Out">
            <ZoomOut size={16} />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={onFitView} title="Fit View">
            <Maximize size={16} />
          </Button>

          <div className="w-px h-6 bg-apex-border-subtle mx-1" />

          <Button variant="ghost" size="icon-sm" onClick={onDelete} disabled={!hasSelection} title="Delete Selected">
            <Trash2 size={16} />
          </Button>

          <div className="w-px h-6 bg-apex-border-subtle mx-1" />

          <Button variant="secondary" size="sm" onClick={onImport} leftIcon={<Upload size={14} />}>
            Import
          </Button>
          <Button variant="secondary" size="sm" onClick={onExport} leftIcon={<Download size={14} />}>
            Export
          </Button>
          <Button variant="secondary" size="sm" onClick={onSave} leftIcon={<Save size={14} />}>
            Save
          </Button>
          <Button variant="primary" size="sm" onClick={onExecute} leftIcon={<Play size={14} />}>
            Execute
          </Button>
        </div>
      </div>

      {/* Node palette */}
      <div className="flex items-center gap-2 bg-apex-bg-secondary rounded-xl border border-apex-border-subtle px-4 py-2.5">
        <span className="text-xs font-medium text-apex-text-secondary mr-2 uppercase tracking-wider">
          Nodes
        </span>

        {NODE_PALETTE.map((type) => {
          const config = NODE_TYPE_CONFIG[type]
          const Icon = ICON_MAP[type]

          return (
            <button
              key={type}
              draggable
              onDragStart={handleDragStart(type)}
              onClick={() => onAddNode(type)}
              className={cn(
                'flex items-center gap-2 px-3 py-2 rounded-lg border transition-all',
                'cursor-grab active:cursor-grabbing',
                'border-apex-border-subtle bg-apex-bg-tertiary',
                'hover:border-apex-border-strong hover:bg-apex-bg-elevated',
                'active:scale-95'
              )}
              title={config.description}
            >
              <div className={cn('p-1 rounded', config.bgColor)}>
                <Icon size={14} className={config.color} />
              </div>
              <span className="text-xs font-medium text-apex-text-primary">
                {config.label}
              </span>
            </button>
          )
        })}
      </div>
    </div>
  )
}
