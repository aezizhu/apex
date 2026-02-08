// ═══════════════════════════════════════════════════════════════════════════════
// Workflow Builder Types
// ═══════════════════════════════════════════════════════════════════════════════

export type WorkflowNodeType = 'task' | 'condition' | 'fork' | 'join' | 'approval'

export interface WorkflowNodeData {
  id: string
  type: WorkflowNodeType
  label: string
  x: number
  y: number
  agentId?: string
  instructions?: string
  maxRetries?: number
  timeoutMs?: number
  // Condition-specific
  conditionExpression?: string
  // Approval-specific
  approvalThreshold?: number
}

export interface WorkflowEdgeData {
  id: string
  source: string
  target: string
  label?: string
  // For condition nodes: which branch this edge represents
  conditionBranch?: 'true' | 'false'
}

export interface WorkflowDAG {
  id?: string
  name: string
  description?: string
  nodes: WorkflowNodeData[]
  edges: WorkflowEdgeData[]
}

export interface DragItem {
  type: WorkflowNodeType
  label: string
}

// Node type display configuration
export const NODE_TYPE_CONFIG: Record<
  WorkflowNodeType,
  {
    label: string
    color: string
    bgColor: string
    borderColor: string
    icon: string
    description: string
  }
> = {
  task: {
    label: 'Task',
    color: 'text-blue-400',
    bgColor: 'bg-blue-500/10',
    borderColor: 'border-blue-500/40',
    icon: 'ListTodo',
    description: 'Execute an agent task',
  },
  condition: {
    label: 'Condition',
    color: 'text-amber-400',
    bgColor: 'bg-amber-500/10',
    borderColor: 'border-amber-500/40',
    icon: 'GitBranch',
    description: 'Branch based on condition',
  },
  fork: {
    label: 'Fork',
    color: 'text-purple-400',
    bgColor: 'bg-purple-500/10',
    borderColor: 'border-purple-500/40',
    icon: 'GitFork',
    description: 'Run tasks in parallel',
  },
  join: {
    label: 'Join',
    color: 'text-green-400',
    bgColor: 'bg-green-500/10',
    borderColor: 'border-green-500/40',
    icon: 'GitMerge',
    description: 'Wait for all branches',
  },
  approval: {
    label: 'Approval Gate',
    color: 'text-red-400',
    bgColor: 'bg-red-500/10',
    borderColor: 'border-red-500/40',
    icon: 'ShieldCheck',
    description: 'Require human approval',
  },
}

export const NODE_WIDTH = 200
export const NODE_HEIGHT = 72
