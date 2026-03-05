import {
  ListTodo,
  GitBranch,
  GitFork,
  GitMerge,
  ShieldCheck,
  X,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input, Textarea } from '@/components/ui/Input'
import { Button } from '@/components/ui/Button'
import { useStore, selectAgentList } from '@/lib/store'
import {
  WorkflowNodeData,
  WorkflowNodeType,
  WorkflowEdgeData,
  NODE_TYPE_CONFIG,
} from './types'

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

// ═══════════════════════════════════════════════════════════════════════════════
// WorkflowProperties Component
// ═══════════════════════════════════════════════════════════════════════════════

interface WorkflowPropertiesProps {
  selectedNode: WorkflowNodeData | null
  selectedEdge: WorkflowEdgeData | null
  onUpdateNode: (id: string, updates: Partial<WorkflowNodeData>) => void
  onUpdateEdge: (id: string, updates: Partial<WorkflowEdgeData>) => void
  onClose: () => void
}

export function WorkflowProperties({
  selectedNode,
  selectedEdge,
  onUpdateNode,
  onUpdateEdge,
  onClose,
}: WorkflowPropertiesProps) {
  const agents = useStore(selectAgentList)

  if (!selectedNode && !selectedEdge) {
    return (
      <div className="w-80 bg-apex-bg-secondary border-l border-apex-border-subtle p-4 flex flex-col items-center justify-center text-center">
        <div className="text-apex-text-tertiary text-sm">
          Select a node or edge to view its properties
        </div>
      </div>
    )
  }

  // Edge properties
  if (selectedEdge) {
    return (
      <div className="w-80 bg-apex-bg-secondary border-l border-apex-border-subtle flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-apex-border-subtle">
          <h3 className="text-sm font-semibold text-apex-text-primary">
            Edge Properties
          </h3>
          <Button variant="ghost" size="icon-xs" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          <Input
            label="Label"
            value={selectedEdge.label || ''}
            onChange={(e) =>
              onUpdateEdge(selectedEdge.id, { label: e.target.value })
            }
            placeholder="Optional edge label"
          />

          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-apex-text-secondary">
              Condition Branch
            </label>
            <div className="flex gap-2">
              {(['', 'true', 'false'] as const).map((branch) => (
                <button
                  key={branch || 'none'}
                  onClick={() =>
                    onUpdateEdge(selectedEdge.id, {
                      conditionBranch: branch || undefined,
                    } as Partial<WorkflowEdgeData>)
                  }
                  className={cn(
                    'px-3 py-1.5 rounded-lg text-xs font-medium transition-all border',
                    selectedEdge.conditionBranch === (branch || undefined)
                      ? 'bg-apex-accent-primary/10 border-apex-accent-primary text-apex-accent-primary'
                      : 'bg-apex-bg-tertiary border-apex-border-subtle text-apex-text-secondary hover:bg-apex-bg-elevated'
                  )}
                >
                  {branch || 'None'}
                </button>
              ))}
            </div>
          </div>

          <div className="text-xs text-apex-text-tertiary pt-2 border-t border-apex-border-subtle">
            <div>Source: {selectedEdge.source.slice(0, 12)}...</div>
            <div>Target: {selectedEdge.target.slice(0, 12)}...</div>
          </div>
        </div>
      </div>
    )
  }

  // Node properties
  if (!selectedNode) return null

  const config = NODE_TYPE_CONFIG[selectedNode.type]
  const Icon = ICON_MAP[selectedNode.type]

  return (
    <div className="w-80 bg-apex-bg-secondary border-l border-apex-border-subtle flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-apex-border-subtle">
        <div className="flex items-center gap-2">
          <div className={cn('p-1.5 rounded-lg', config.bgColor)}>
            <Icon size={14} className={config.color} />
          </div>
          <h3 className="text-sm font-semibold text-apex-text-primary">
            {config.label} Properties
          </h3>
        </div>
        <Button variant="ghost" size="icon-xs" onClick={onClose}>
          <X size={14} />
        </Button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Name */}
        <Input
          label="Name"
          value={selectedNode.label}
          onChange={(e) =>
            onUpdateNode(selectedNode.id, { label: e.target.value })
          }
          placeholder="Node name"
        />

        {/* Agent assignment (for task and approval nodes) */}
        {(selectedNode.type === 'task' || selectedNode.type === 'approval') && (
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-apex-text-secondary">
              Assigned Agent
            </label>
            <select
              value={selectedNode.agentId || ''}
              onChange={(e) =>
                onUpdateNode(selectedNode.id, {
                  agentId: e.target.value || undefined,
                })
              }
              className="w-full h-10 px-3 rounded-lg text-sm bg-apex-bg-tertiary border border-apex-border-subtle text-apex-text-primary focus:border-apex-accent-primary focus:ring-1 focus:ring-apex-accent-primary outline-none"
            >
              <option value="">Auto-assign</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name} ({agent.model})
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Instructions (for task nodes) */}
        {selectedNode.type === 'task' && (
          <Textarea
            label="Instructions"
            value={selectedNode.instructions || ''}
            onChange={(e) =>
              onUpdateNode(selectedNode.id, { instructions: e.target.value })
            }
            placeholder="Task instructions for the agent..."
            rows={5}
          />
        )}

        {/* Condition expression */}
        {selectedNode.type === 'condition' && (
          <Textarea
            label="Condition Expression"
            value={selectedNode.conditionExpression || ''}
            onChange={(e) =>
              onUpdateNode(selectedNode.id, {
                conditionExpression: e.target.value,
              })
            }
            placeholder="e.g., result.status === 'success'"
            rows={3}
            hint="JavaScript expression evaluated against the previous task output"
          />
        )}

        {/* Approval threshold */}
        {selectedNode.type === 'approval' && (
          <Input
            label="Risk Threshold"
            type="number"
            min={0}
            max={1}
            step={0.1}
            value={selectedNode.approvalThreshold ?? 0.7}
            onChange={(e) =>
              onUpdateNode(selectedNode.id, {
                approvalThreshold: parseFloat(e.target.value),
              })
            }
            hint="Actions above this risk score require approval"
          />
        )}

        {/* Limits section */}
        <div className="pt-3 border-t border-apex-border-subtle space-y-4">
          <h4 className="text-xs font-medium text-apex-text-tertiary uppercase tracking-wider">
            Limits
          </h4>

          <Input
            label="Max Retries"
            type="number"
            min={0}
            max={10}
            value={selectedNode.maxRetries ?? 3}
            onChange={(e) =>
              onUpdateNode(selectedNode.id, {
                maxRetries: parseInt(e.target.value, 10),
              })
            }
          />

          <Input
            label="Timeout (ms)"
            type="number"
            min={0}
            step={1000}
            value={selectedNode.timeoutMs ?? 60000}
            onChange={(e) =>
              onUpdateNode(selectedNode.id, {
                timeoutMs: parseInt(e.target.value, 10),
              })
            }
            hint="0 for no timeout"
          />
        </div>

        {/* Node ID (read-only info) */}
        <div className="pt-3 border-t border-apex-border-subtle">
          <div className="text-xs text-apex-text-tertiary">
            <span className="font-medium">ID:</span>{' '}
            <code className="bg-apex-bg-tertiary px-1.5 py-0.5 rounded text-[10px]">
              {selectedNode.id}
            </code>
          </div>
          <div className="text-xs text-apex-text-tertiary mt-1">
            <span className="font-medium">Position:</span> ({Math.round(selectedNode.x)}, {Math.round(selectedNode.y)})
          </div>
        </div>
      </div>
    </div>
  )
}
