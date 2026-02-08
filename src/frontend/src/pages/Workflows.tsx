import { useCallback } from 'react'
import { WorkflowBuilder } from '../components/workflow/WorkflowBuilder'
import type { WorkflowDAG } from '../components/workflow/types'
import toast from 'react-hot-toast'
import { dagApi } from '../lib/api'

export default function Workflows() {
  const buildTasks = useCallback(
    (dag: WorkflowDAG) =>
      dag.nodes
        .filter((n) => n.type === 'task')
        .map((node) => {
          const deps = dag.edges
            .filter((e) => e.target === node.id)
            .map((e) => e.source)
          return {
            name: node.label || node.id,
            prompt: node.instructions || '',
            dependencies: deps.length > 0 ? deps : undefined,
          }
        }),
    []
  )

  const handleSave = useCallback(
    async (dag: WorkflowDAG) => {
      try {
        await dagApi.create({ name: dag.name, tasks: buildTasks(dag) })
        toast.success('Workflow saved')
      } catch {
        toast.error('Failed to save workflow')
      }
    },
    [buildTasks]
  )

  const handleExecute = useCallback(
    async (dag: WorkflowDAG) => {
      try {
        await dagApi.create({ name: dag.name, tasks: buildTasks(dag) })
        toast.success('Workflow submitted for execution')
      } catch {
        toast.error('Failed to execute workflow')
      }
    },
    [buildTasks]
  )

  return (
    <div className="h-[calc(100vh-8rem)]">
      <WorkflowBuilder onSave={handleSave} onExecute={handleExecute} />
    </div>
  )
}
