import { memo, useCallback } from 'react'
import {
  ListTodo,
  GitBranch,
  GitFork,
  GitMerge,
  ShieldCheck,
  GripVertical,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { WorkflowNodeData, WorkflowNodeType, NODE_TYPE_CONFIG, NODE_WIDTH, NODE_HEIGHT } from './types'

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
// WorkflowNode Component
// ═══════════════════════════════════════════════════════════════════════════════

interface WorkflowNodeProps {
  node: WorkflowNodeData
  selected: boolean
  connecting: boolean
  onSelect: (id: string) => void
  onStartConnect: (id: string) => void
  onEndConnect: (id: string) => void
  onDragStart: (id: string, e: React.MouseEvent) => void
}

function WorkflowNodeComponent({
  node,
  selected,
  connecting,
  onSelect,
  onStartConnect,
  onEndConnect,
  onDragStart,
}: WorkflowNodeProps) {
  const config = NODE_TYPE_CONFIG[node.type]
  const Icon = ICON_MAP[node.type]

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return
      e.stopPropagation()
      onSelect(node.id)
      onDragStart(node.id, e)
    },
    [node.id, onSelect, onDragStart]
  )

  const handleConnectorMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      onStartConnect(node.id)
    },
    [node.id, onStartConnect]
  )

  const handleConnectorMouseUp = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      onEndConnect(node.id)
    },
    [node.id, onEndConnect]
  )

  return (
    <g
      transform={`translate(${node.x}, ${node.y})`}
      className="cursor-grab active:cursor-grabbing"
      onMouseDown={handleMouseDown}
    >
      {/* Shadow */}
      <rect
        x={2}
        y={2}
        width={NODE_WIDTH}
        height={NODE_HEIGHT}
        rx={12}
        fill="rgba(0,0,0,0.3)"
      />

      {/* Node body */}
      <rect
        width={NODE_WIDTH}
        height={NODE_HEIGHT}
        rx={12}
        className={cn(
          'transition-all duration-150',
          selected
            ? 'fill-[#1e293b] stroke-[#3b82f6] stroke-[2px]'
            : 'fill-[#1a1a2e] stroke-[#2a2a3e] stroke-[1px] hover:stroke-[#3a3a4e]'
        )}
      />

      {/* Type indicator stripe */}
      <rect
        x={0}
        y={0}
        width={4}
        height={NODE_HEIGHT}
        rx={2}
        className={cn(
          node.type === 'task' && 'fill-blue-500',
          node.type === 'condition' && 'fill-amber-500',
          node.type === 'fork' && 'fill-purple-500',
          node.type === 'join' && 'fill-green-500',
          node.type === 'approval' && 'fill-red-500'
        )}
      />

      {/* Drag handle */}
      <foreignObject x={8} y={(NODE_HEIGHT - 16) / 2} width={16} height={16}>
        <GripVertical size={16} className="text-gray-600" />
      </foreignObject>

      {/* Icon */}
      <foreignObject x={28} y={(NODE_HEIGHT - 32) / 2} width={32} height={32}>
        <div
          className={cn(
            'w-8 h-8 rounded-lg flex items-center justify-center',
            config.bgColor
          )}
        >
          <Icon size={16} className={config.color} />
        </div>
      </foreignObject>

      {/* Label */}
      <foreignObject x={68} y={12} width={NODE_WIDTH - 80} height={20}>
        <div className="text-sm font-medium text-white truncate leading-5">
          {node.label}
        </div>
      </foreignObject>

      {/* Type badge */}
      <foreignObject x={68} y={34} width={NODE_WIDTH - 80} height={20}>
        <div className={cn('text-xs truncate leading-5', config.color)}>
          {config.label}
          {node.agentId && (
            <span className="text-gray-500 ml-1">
              {' '}| {node.agentId.slice(0, 8)}
            </span>
          )}
        </div>
      </foreignObject>

      {/* Input connector (left) */}
      <circle
        cx={0}
        cy={NODE_HEIGHT / 2}
        r={connecting ? 8 : 6}
        className={cn(
          'transition-all duration-150 cursor-crosshair',
          connecting
            ? 'fill-blue-500 stroke-blue-400 stroke-[2px]'
            : 'fill-[#2a2a3e] stroke-[#3a3a4e] stroke-[1px] hover:fill-blue-500/50 hover:stroke-blue-400'
        )}
        onMouseUp={handleConnectorMouseUp}
      />

      {/* Output connector (right) */}
      <circle
        cx={NODE_WIDTH}
        cy={NODE_HEIGHT / 2}
        r={connecting ? 8 : 6}
        className={cn(
          'transition-all duration-150 cursor-crosshair',
          connecting
            ? 'fill-blue-500 stroke-blue-400 stroke-[2px]'
            : 'fill-[#2a2a3e] stroke-[#3a3a4e] stroke-[1px] hover:fill-blue-500/50 hover:stroke-blue-400'
        )}
        onMouseDown={handleConnectorMouseDown}
      />

      {/* Condition: extra connectors for true/false branches */}
      {node.type === 'condition' && (
        <>
          <circle
            cx={NODE_WIDTH / 2}
            cy={NODE_HEIGHT}
            r={connecting ? 8 : 6}
            className="fill-amber-500/30 stroke-amber-400 stroke-[1px] cursor-crosshair hover:fill-amber-500/50"
            onMouseDown={handleConnectorMouseDown}
          />
          <foreignObject
            x={NODE_WIDTH / 2 + 10}
            y={NODE_HEIGHT - 6}
            width={30}
            height={14}
          >
            <span className="text-[10px] text-amber-400">false</span>
          </foreignObject>
        </>
      )}
    </g>
  )
}

export const WorkflowNode = memo(WorkflowNodeComponent)
