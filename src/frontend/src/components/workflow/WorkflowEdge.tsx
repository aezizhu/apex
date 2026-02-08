import { memo } from 'react'
import { cn } from '@/lib/utils'
import { WorkflowEdgeData, WorkflowNodeData, NODE_WIDTH, NODE_HEIGHT } from './types'

// ═══════════════════════════════════════════════════════════════════════════════
// WorkflowEdge Component
// ═══════════════════════════════════════════════════════════════════════════════

interface WorkflowEdgeProps {
  edge: WorkflowEdgeData
  sourceNode: WorkflowNodeData
  targetNode: WorkflowNodeData
  selected: boolean
  onSelect: (id: string) => void
}

function WorkflowEdgeComponent({
  edge,
  sourceNode,
  targetNode,
  selected,
  onSelect,
}: WorkflowEdgeProps) {
  // Calculate connection points
  const sourceX = sourceNode.x + NODE_WIDTH
  const sourceY = sourceNode.y + NODE_HEIGHT / 2
  const targetX = targetNode.x
  const targetY = targetNode.y + NODE_HEIGHT / 2

  // For condition false branch, connect from the bottom connector
  const isConditionFalse = edge.conditionBranch === 'false'
  const actualSourceX = isConditionFalse
    ? sourceNode.x + NODE_WIDTH / 2
    : sourceX
  const actualSourceY = isConditionFalse
    ? sourceNode.y + NODE_HEIGHT
    : sourceY

  // Calculate bezier control points for a smooth curve
  const dx = targetX - actualSourceX
  const dy = targetY - actualSourceY
  const cpOffset = Math.max(Math.abs(dx) * 0.4, 50)

  let path: string
  if (isConditionFalse) {
    // Route downward first, then to target
    const midY = actualSourceY + Math.max(Math.abs(dy) * 0.5, 40)
    path = `M ${actualSourceX} ${actualSourceY} C ${actualSourceX} ${midY}, ${targetX} ${midY}, ${targetX} ${targetY}`
  } else {
    // Standard horizontal bezier
    path = `M ${actualSourceX} ${actualSourceY} C ${actualSourceX + cpOffset} ${actualSourceY}, ${targetX - cpOffset} ${targetY}, ${targetX} ${targetY}`
  }

  // Arrow marker
  const arrowId = `arrow-${edge.id}`
  const midX = (actualSourceX + targetX) / 2
  const midY2 = (actualSourceY + targetY) / 2

  return (
    <g
      onClick={(e) => {
        e.stopPropagation()
        onSelect(edge.id)
      }}
      className="cursor-pointer"
    >
      {/* Arrow marker definition */}
      <defs>
        <marker
          id={arrowId}
          viewBox="0 0 10 7"
          refX="10"
          refY="3.5"
          markerWidth={8}
          markerHeight={8}
          orient="auto-start-reverse"
        >
          <polygon
            points="0 0, 10 3.5, 0 7"
            className={cn(
              selected ? 'fill-blue-400' : 'fill-gray-500',
              edge.conditionBranch === 'true' && 'fill-green-400',
              edge.conditionBranch === 'false' && 'fill-amber-400'
            )}
          />
        </marker>
      </defs>

      {/* Invisible wide path for easier clicking */}
      <path
        d={path}
        fill="none"
        stroke="transparent"
        strokeWidth={16}
      />

      {/* Visible edge path */}
      <path
        d={path}
        fill="none"
        className={cn(
          'transition-all duration-150',
          selected
            ? 'stroke-blue-400'
            : edge.conditionBranch === 'true'
            ? 'stroke-green-500/50'
            : edge.conditionBranch === 'false'
            ? 'stroke-amber-500/50'
            : 'stroke-gray-600 hover:stroke-gray-400'
        )}
        strokeWidth={selected ? 2.5 : 1.5}
        strokeDasharray={edge.conditionBranch ? '6 3' : undefined}
        markerEnd={`url(#${arrowId})`}
      />

      {/* Edge label */}
      {edge.label && (
        <g transform={`translate(${midX}, ${midY2})`}>
          <rect
            x={-30}
            y={-10}
            width={60}
            height={20}
            rx={4}
            className="fill-[#1a1a2e] stroke-[#2a2a3e] stroke-[1px]"
          />
          <text
            textAnchor="middle"
            dominantBaseline="central"
            className="text-[10px] fill-gray-400 select-none"
          >
            {edge.label}
          </text>
        </g>
      )}

      {/* Condition branch label */}
      {edge.conditionBranch && (
        <g transform={`translate(${actualSourceX + (targetX - actualSourceX) * 0.2}, ${actualSourceY + (targetY - actualSourceY) * 0.2 - 12})`}>
          <text
            textAnchor="middle"
            className={cn(
              'text-[10px] font-medium select-none',
              edge.conditionBranch === 'true' ? 'fill-green-400' : 'fill-amber-400'
            )}
          >
            {edge.conditionBranch}
          </text>
        </g>
      )}
    </g>
  )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Temporary Edge (while user is drawing a connection)
// ═══════════════════════════════════════════════════════════════════════════════

interface TempEdgeProps {
  sourceX: number
  sourceY: number
  targetX: number
  targetY: number
}

export function TempEdge({ sourceX, sourceY, targetX, targetY }: TempEdgeProps) {
  const cpOffset = Math.max(Math.abs(targetX - sourceX) * 0.4, 50)
  const path = `M ${sourceX} ${sourceY} C ${sourceX + cpOffset} ${sourceY}, ${targetX - cpOffset} ${targetY}, ${targetX} ${targetY}`

  return (
    <path
      d={path}
      fill="none"
      className="stroke-blue-400/60"
      strokeWidth={2}
      strokeDasharray="8 4"
      pointerEvents="none"
    />
  )
}

export const WorkflowEdge = memo(WorkflowEdgeComponent)
