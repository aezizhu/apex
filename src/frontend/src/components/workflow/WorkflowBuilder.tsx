import { useCallback, useEffect, useRef, useState } from 'react'
import * as d3 from 'd3'
import toast from 'react-hot-toast'
import { WorkflowNode } from './WorkflowNode'
import { WorkflowEdge, TempEdge } from './WorkflowEdge'
import { WorkflowToolbar } from './WorkflowToolbar'
import { WorkflowProperties } from './WorkflowProperties'
import {
  WorkflowNodeData,
  WorkflowNodeType,
  WorkflowEdgeData,
  WorkflowDAG,
  NODE_WIDTH,
  NODE_HEIGHT,
  NODE_TYPE_CONFIG,
} from './types'

// ═══════════════════════════════════════════════════════════════════════════════
// History entry for undo/redo
// ═══════════════════════════════════════════════════════════════════════════════

interface HistoryEntry {
  nodes: WorkflowNodeData[]
  edges: WorkflowEdgeData[]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility: generate unique ID
// ═══════════════════════════════════════════════════════════════════════════════

let nodeCounter = 0
function generateId(prefix: string): string {
  nodeCounter += 1
  return `${prefix}-${Date.now()}-${nodeCounter}`
}

// ═══════════════════════════════════════════════════════════════════════════════
// WorkflowBuilder Component
// ═══════════════════════════════════════════════════════════════════════════════

interface WorkflowBuilderProps {
  initialDAG?: WorkflowDAG
  onSave?: (dag: WorkflowDAG) => void
  onExecute?: (dag: WorkflowDAG) => void
}

export function WorkflowBuilder({ initialDAG, onSave, onExecute }: WorkflowBuilderProps) {
  // ── State ──────────────────────────────────────────────────────────────────
  const [workflowName, setWorkflowName] = useState(initialDAG?.name || 'Untitled Workflow')
  const [nodes, setNodes] = useState<WorkflowNodeData[]>(initialDAG?.nodes || [])
  const [edges, setEdges] = useState<WorkflowEdgeData[]>(initialDAG?.edges || [])
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null)

  // Drag state
  const [dragNodeId, setDragNodeId] = useState<string | null>(null)
  const dragOffset = useRef({ x: 0, y: 0 })

  // Connection drawing state
  const [connectingFrom, setConnectingFrom] = useState<string | null>(null)
  const [tempEdgeTarget, setTempEdgeTarget] = useState<{ x: number; y: number } | null>(null)

  // SVG zoom/pan via d3
  const svgRef = useRef<SVGSVGElement>(null)
  const gRef = useRef<SVGGElement>(null)
  const zoomRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown>>()
  const currentTransform = useRef(d3.zoomIdentity)

  // Undo/redo history
  const [history, setHistory] = useState<HistoryEntry[]>([{ nodes: [], edges: [] }])
  const [historyIndex, setHistoryIndex] = useState(0)
  const isUndoRedo = useRef(false)

  // ── Import initial DAG ─────────────────────────────────────────────────────
  useEffect(() => {
    if (initialDAG) {
      setWorkflowName(initialDAG.name)
      setNodes(initialDAG.nodes)
      setEdges(initialDAG.edges)
    }
  }, [initialDAG])

  // ── History tracking ───────────────────────────────────────────────────────
  useEffect(() => {
    if (isUndoRedo.current) {
      isUndoRedo.current = false
      return
    }
    setHistory((prev) => {
      const trimmed = prev.slice(0, historyIndex + 1)
      return [...trimmed, { nodes: [...nodes], edges: [...edges] }]
    })
    setHistoryIndex((prev) => prev + 1)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, edges])

  // ── D3 Zoom Setup ─────────────────────────────────────────────────────────
  useEffect(() => {
    if (!svgRef.current || !gRef.current) return

    const svg = d3.select(svgRef.current)
    const g = d3.select(gRef.current)

    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.15, 3])
      .on('zoom', (event: d3.D3ZoomEvent<SVGSVGElement, unknown>) => {
        currentTransform.current = event.transform
        g.attr('transform', event.transform.toString())
      })

    svg.call(zoom)
    zoomRef.current = zoom

    // Disable double-click zoom
    svg.on('dblclick.zoom', null)

    return () => {
      svg.on('.zoom', null)
    }
  }, [])

  // ── Zoom Controls ──────────────────────────────────────────────────────────
  const handleZoomIn = useCallback(() => {
    if (!svgRef.current || !zoomRef.current) return
    d3.select(svgRef.current)
      .transition()
      .duration(300)
      .call(zoomRef.current.scaleBy, 1.3)
  }, [])

  const handleZoomOut = useCallback(() => {
    if (!svgRef.current || !zoomRef.current) return
    d3.select(svgRef.current)
      .transition()
      .duration(300)
      .call(zoomRef.current.scaleBy, 0.7)
  }, [])

  const handleFitView = useCallback(() => {
    if (!svgRef.current || !zoomRef.current || nodes.length === 0) return

    const svg = svgRef.current
    const width = svg.clientWidth
    const height = svg.clientHeight

    const minX = Math.min(...nodes.map((n) => n.x)) - 50
    const minY = Math.min(...nodes.map((n) => n.y)) - 50
    const maxX = Math.max(...nodes.map((n) => n.x + NODE_WIDTH)) + 50
    const maxY = Math.max(...nodes.map((n) => n.y + NODE_HEIGHT)) + 50

    const graphWidth = maxX - minX
    const graphHeight = maxY - minY

    const scale = Math.min(width / graphWidth, height / graphHeight, 1.5)
    const tx = (width - graphWidth * scale) / 2 - minX * scale
    const ty = (height - graphHeight * scale) / 2 - minY * scale

    d3.select(svgRef.current)
      .transition()
      .duration(500)
      .call(zoomRef.current.transform, d3.zoomIdentity.translate(tx, ty).scale(scale))
  }, [nodes])

  // ── Undo/Redo ─────────────────────────────────────────────────────────────
  const handleUndo = useCallback(() => {
    if (historyIndex <= 0) return
    const entry = history[historyIndex - 1]
    if (!entry) return
    isUndoRedo.current = true
    setNodes([...entry.nodes])
    setEdges([...entry.edges])
    setHistoryIndex((i) => i - 1)
  }, [history, historyIndex])

  const handleRedo = useCallback(() => {
    if (historyIndex >= history.length - 1) return
    const entry = history[historyIndex + 1]
    if (!entry) return
    isUndoRedo.current = true
    setNodes([...entry.nodes])
    setEdges([...entry.edges])
    setHistoryIndex((i) => i + 1)
  }, [history, historyIndex])

  // ── Add Node ──────────────────────────────────────────────────────────────
  const addNode = useCallback(
    (type: WorkflowNodeType, x?: number, y?: number) => {
      const config = NODE_TYPE_CONFIG[type]
      const newNode: WorkflowNodeData = {
        id: generateId(type),
        type,
        label: `${config.label} ${nodes.filter((n) => n.type === type).length + 1}`,
        x: x ?? 100 + nodes.length * 60,
        y: y ?? 200 + (nodes.length % 3) * 120,
        maxRetries: 3,
        timeoutMs: 60000,
      }

      if (type === 'approval') {
        newNode.approvalThreshold = 0.7
      }

      setNodes((prev) => [...prev, newNode])
      setSelectedNodeId(newNode.id)
      setSelectedEdgeId(null)
    },
    [nodes]
  )

  // ── Delete Selection ──────────────────────────────────────────────────────
  const handleDelete = useCallback(() => {
    if (selectedNodeId) {
      setNodes((prev) => prev.filter((n) => n.id !== selectedNodeId))
      setEdges((prev) =>
        prev.filter((e) => e.source !== selectedNodeId && e.target !== selectedNodeId)
      )
      setSelectedNodeId(null)
    } else if (selectedEdgeId) {
      setEdges((prev) => prev.filter((e) => e.id !== selectedEdgeId))
      setSelectedEdgeId(null)
    }
  }, [selectedNodeId, selectedEdgeId])

  // ── Node Selection ────────────────────────────────────────────────────────
  const handleSelectNode = useCallback((id: string) => {
    setSelectedNodeId(id)
    setSelectedEdgeId(null)
  }, [])

  const handleSelectEdge = useCallback((id: string) => {
    setSelectedEdgeId(id)
    setSelectedNodeId(null)
  }, [])

  // ── Node Dragging ─────────────────────────────────────────────────────────
  const handleDragStart = useCallback(
    (nodeId: string, e: React.MouseEvent) => {
      const node = nodes.find((n) => n.id === nodeId)
      if (!node) return

      const point = svgRef.current!.createSVGPoint()
      point.x = e.clientX
      point.y = e.clientY
      const ctm = gRef.current!.getScreenCTM()!.inverse()
      const svgPoint = point.matrixTransform(ctm)

      dragOffset.current = {
        x: svgPoint.x - node.x,
        y: svgPoint.y - node.y,
      }
      setDragNodeId(nodeId)
    },
    [nodes]
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!svgRef.current || !gRef.current) return

      const point = svgRef.current.createSVGPoint()
      point.x = e.clientX
      point.y = e.clientY
      const ctm = gRef.current.getScreenCTM()!.inverse()
      const svgPoint = point.matrixTransform(ctm)

      // Handle node dragging
      if (dragNodeId) {
        const newX = svgPoint.x - dragOffset.current.x
        const newY = svgPoint.y - dragOffset.current.y

        setNodes((prev) =>
          prev.map((n) =>
            n.id === dragNodeId ? { ...n, x: newX, y: newY } : n
          )
        )
      }

      // Handle connection drawing
      if (connectingFrom) {
        setTempEdgeTarget({ x: svgPoint.x, y: svgPoint.y })
      }
    },
    [dragNodeId, connectingFrom]
  )

  const handleMouseUp = useCallback(() => {
    setDragNodeId(null)
    if (connectingFrom && !tempEdgeTarget) {
      setConnectingFrom(null)
    }
    // If we were connecting and dropped on empty space, cancel
    if (connectingFrom) {
      setConnectingFrom(null)
      setTempEdgeTarget(null)
    }
  }, [connectingFrom, tempEdgeTarget])

  // ── Connection Drawing ────────────────────────────────────────────────────
  const handleStartConnect = useCallback((nodeId: string) => {
    setConnectingFrom(nodeId)
    setTempEdgeTarget(null)
  }, [])

  const handleEndConnect = useCallback(
    (targetId: string) => {
      if (!connectingFrom || connectingFrom === targetId) {
        setConnectingFrom(null)
        setTempEdgeTarget(null)
        return
      }

      // Check for duplicate edges
      const exists = edges.some(
        (e) => e.source === connectingFrom && e.target === targetId
      )
      if (exists) {
        toast.error('Connection already exists')
        setConnectingFrom(null)
        setTempEdgeTarget(null)
        return
      }

      // Check for self-loops
      if (connectingFrom === targetId) {
        toast.error('Cannot connect a node to itself')
        setConnectingFrom(null)
        setTempEdgeTarget(null)
        return
      }

      const sourceNode = nodes.find((n) => n.id === connectingFrom)
      const newEdge: WorkflowEdgeData = {
        id: generateId('edge'),
        source: connectingFrom,
        target: targetId,
        // Auto-label condition branches
        conditionBranch: sourceNode?.type === 'condition' ? 'true' : undefined,
      }

      setEdges((prev) => [...prev, newEdge])
      setConnectingFrom(null)
      setTempEdgeTarget(null)
    },
    [connectingFrom, edges, nodes]
  )

  // ── Node/Edge Update ──────────────────────────────────────────────────────
  const handleUpdateNode = useCallback(
    (id: string, updates: Partial<WorkflowNodeData>) => {
      setNodes((prev) =>
        prev.map((n) => (n.id === id ? { ...n, ...updates } : n))
      )
    },
    []
  )

  const handleUpdateEdge = useCallback(
    (id: string, updates: Partial<WorkflowEdgeData>) => {
      setEdges((prev) =>
        prev.map((e) => (e.id === id ? { ...e, ...updates } : e))
      )
    },
    []
  )

  // ── Export/Import ─────────────────────────────────────────────────────────
  const buildDAG = useCallback((): WorkflowDAG => {
    return {
      name: workflowName,
      nodes,
      edges,
    }
  }, [workflowName, nodes, edges])

  const handleExport = useCallback(() => {
    const dag = buildDAG()
    const json = JSON.stringify(dag, null, 2)
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${workflowName.replace(/\s+/g, '-').toLowerCase()}.json`
    a.click()
    URL.revokeObjectURL(url)
    toast.success('Workflow exported')
  }, [buildDAG, workflowName])

  const handleImport = useCallback(() => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = '.json'
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0]
      if (!file) return

      const reader = new FileReader()
      reader.onload = (evt) => {
        try {
          const dag: WorkflowDAG = JSON.parse(evt.target?.result as string)

          if (!dag.nodes || !dag.edges) {
            toast.error('Invalid workflow file: missing nodes or edges')
            return
          }

          setWorkflowName(dag.name || 'Imported Workflow')
          setNodes(dag.nodes)
          setEdges(dag.edges)
          setSelectedNodeId(null)
          setSelectedEdgeId(null)
          toast.success('Workflow imported')

          // Fit view after import
          setTimeout(() => handleFitView(), 100)
        } catch {
          toast.error('Invalid JSON file')
        }
      }
      reader.readAsText(file)
    }
    input.click()
  }, [handleFitView])

  const handleSave = useCallback(() => {
    const dag = buildDAG()
    onSave?.(dag)
    toast.success('Workflow saved')
  }, [buildDAG, onSave])

  const handleExecute = useCallback(() => {
    if (nodes.length === 0) {
      toast.error('Add at least one node before executing')
      return
    }
    const dag = buildDAG()
    onExecute?.(dag)
    toast.success('Workflow submitted for execution')
  }, [buildDAG, nodes, onExecute])

  // ── Drop handler for drag-and-drop from toolbar ───────────────────────────
  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      const type = e.dataTransfer.getData('workflow-node-type') as WorkflowNodeType
      if (!type) return

      if (!svgRef.current || !gRef.current) return

      const point = svgRef.current.createSVGPoint()
      point.x = e.clientX
      point.y = e.clientY
      const ctm = gRef.current.getScreenCTM()!.inverse()
      const svgPoint = point.matrixTransform(ctm)

      addNode(type, svgPoint.x - NODE_WIDTH / 2, svgPoint.y - NODE_HEIGHT / 2)
    },
    [addNode]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'copy'
  }, [])

  // ── Canvas click (deselect) ───────────────────────────────────────────────
  const handleCanvasClick = useCallback(() => {
    setSelectedNodeId(null)
    setSelectedEdgeId(null)
  }, [])

  // ── Keyboard shortcuts ────────────────────────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Delete/Backspace: delete selection
      if ((e.key === 'Delete' || e.key === 'Backspace') && (selectedNodeId || selectedEdgeId)) {
        // Don't delete if user is typing in an input
        if ((e.target as HTMLElement).tagName === 'INPUT' || (e.target as HTMLElement).tagName === 'TEXTAREA') {
          return
        }
        handleDelete()
      }

      // Ctrl/Cmd+Z: undo
      if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
        e.preventDefault()
        handleUndo()
      }

      // Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y: redo
      if (((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'z') || ((e.ctrlKey || e.metaKey) && e.key === 'y')) {
        e.preventDefault()
        handleRedo()
      }

      // Escape: deselect / cancel connection
      if (e.key === 'Escape') {
        setSelectedNodeId(null)
        setSelectedEdgeId(null)
        setConnectingFrom(null)
        setTempEdgeTarget(null)
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [selectedNodeId, selectedEdgeId, handleDelete, handleUndo, handleRedo])

  // ── Derived state ─────────────────────────────────────────────────────────
  const selectedNode = selectedNodeId ? nodes.find((n) => n.id === selectedNodeId) || null : null
  const selectedEdge = selectedEdgeId ? edges.find((e) => e.id === selectedEdgeId) || null : null
  const connectingSourceNode = connectingFrom ? nodes.find((n) => n.id === connectingFrom) : null

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <WorkflowToolbar
        onAddNode={addNode}
        onDelete={handleDelete}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onFitView={handleFitView}
        onUndo={handleUndo}
        onRedo={handleRedo}
        onExport={handleExport}
        onImport={handleImport}
        onSave={handleSave}
        onExecute={handleExecute}
        canUndo={historyIndex > 0}
        canRedo={historyIndex < history.length - 1}
        hasSelection={!!(selectedNodeId || selectedEdgeId)}
        workflowName={workflowName}
        onNameChange={setWorkflowName}
      />

      {/* Canvas + Properties */}
      <div className="flex flex-1 min-h-0 mt-3 rounded-xl overflow-hidden border border-apex-border-subtle">
        {/* SVG Canvas */}
        <div className="flex-1 relative bg-apex-bg-primary">
          {/* Grid pattern background */}
          <div
            className="absolute inset-0 opacity-[0.03]"
            style={{
              backgroundImage:
                'radial-gradient(circle, #ffffff 1px, transparent 1px)',
              backgroundSize: '24px 24px',
            }}
          />

          <svg
            ref={svgRef}
            className="w-full h-full"
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
            onClick={handleCanvasClick}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
          >
            <g ref={gRef}>
              {/* Edges */}
              {edges.map((edge) => {
                const sourceNode = nodes.find((n) => n.id === edge.source)
                const targetNode = nodes.find((n) => n.id === edge.target)
                if (!sourceNode || !targetNode) return null

                return (
                  <WorkflowEdge
                    key={edge.id}
                    edge={edge}
                    sourceNode={sourceNode}
                    targetNode={targetNode}
                    selected={edge.id === selectedEdgeId}
                    onSelect={handleSelectEdge}
                  />
                )
              })}

              {/* Temp edge while connecting */}
              {connectingFrom && connectingSourceNode && tempEdgeTarget && (
                <TempEdge
                  sourceX={connectingSourceNode.x + NODE_WIDTH}
                  sourceY={connectingSourceNode.y + NODE_HEIGHT / 2}
                  targetX={tempEdgeTarget.x}
                  targetY={tempEdgeTarget.y}
                />
              )}

              {/* Nodes */}
              {nodes.map((node) => (
                <WorkflowNode
                  key={node.id}
                  node={node}
                  selected={node.id === selectedNodeId}
                  connecting={!!connectingFrom}
                  onSelect={handleSelectNode}
                  onStartConnect={handleStartConnect}
                  onEndConnect={handleEndConnect}
                  onDragStart={handleDragStart}
                />
              ))}
            </g>
          </svg>

          {/* Empty state */}
          {nodes.length === 0 && (
            <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
              <div className="text-apex-text-tertiary text-lg font-medium mb-2">
                Build Your Workflow
              </div>
              <div className="text-apex-text-muted text-sm max-w-md text-center">
                Drag nodes from the toolbar above or click a node type to add it.
                Connect nodes by dragging from the right connector to the left connector of another node.
              </div>
            </div>
          )}
        </div>

        {/* Properties Panel */}
        {(selectedNode || selectedEdge) && (
          <WorkflowProperties
            selectedNode={selectedNode}
            selectedEdge={selectedEdge}
            onUpdateNode={handleUpdateNode}
            onUpdateEdge={handleUpdateEdge}
            onClose={() => {
              setSelectedNodeId(null)
              setSelectedEdgeId(null)
            }}
          />
        )}
      </div>
    </div>
  )
}
