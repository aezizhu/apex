import { useEffect, useRef } from 'react'
import { useStore } from '../lib/store'
import { agentApi, taskApi, metricsApi, approvalApi } from '../lib/api'
import type { Agent, Task } from '../lib/store'

/**
 * Fetches initial data from the backend API and populates the Zustand store.
 * Also sets up periodic polling for metrics (every 30s).
 */
export function useInitialData() {
  const { setAgents, setTasks, setMetrics, setApprovals } = useStore()
  const hasFetched = useRef(false)

  useEffect(() => {
    if (hasFetched.current) return
    hasFetched.current = true

    async function fetchInitialData() {
      // Fetch agents
      try {
        const agentsResponse = await agentApi.listRaw()
        const agentsData = agentsResponse.data
        if (Array.isArray(agentsData)) {
          const agents: Agent[] = agentsData.map((a) => ({
            id: a.id,
            name: a.name,
            model: a.model,
            status: a.status,
            currentLoad: a.currentLoad,
            maxLoad: a.maxLoad,
            successRate: a.successRate,
            reputationScore: a.reputationScore,
            totalTokens: a.totalTokens,
            totalCost: a.totalCost,
            confidence: a.confidence,
          }))
          setAgents(agents)
        }
      } catch (err) {
        console.warn('[Init] Failed to fetch agents:', err)
      }

      // Fetch tasks
      try {
        const tasksResponse = await taskApi.list({ pageSize: 200 })
        const tasksData = tasksResponse.data
        if (Array.isArray(tasksData)) {
          const tasks: Task[] = tasksData.map((t) => ({
            id: t.id,
            dagId: t.dagId,
            name: t.name,
            status: t.status,
            agentId: t.agentId,
            tokensUsed: t.tokensUsed,
            costDollars: t.costDollars,
            createdAt: t.createdAt,
            startedAt: t.startedAt,
            completedAt: t.completedAt,
          }))
          setTasks(tasks)
        }
      } catch (err) {
        console.warn('[Init] Failed to fetch tasks:', err)
      }

      // Fetch metrics
      try {
        const metricsResponse = await metricsApi.getSystem()
        if (metricsResponse.data) {
          setMetrics(metricsResponse.data)
        }
      } catch (err) {
        console.warn('[Init] Failed to fetch metrics:', err)
      }

      // Fetch approvals
      try {
        const approvalsResponse = await approvalApi.list({ pageSize: 100 })
        const approvalsData = approvalsResponse.data
        if (Array.isArray(approvalsData)) {
          setApprovals(approvalsData)
        }
      } catch (err) {
        console.warn('[Init] Failed to fetch approvals:', err)
      }
    }

    fetchInitialData()
  }, [setAgents, setTasks, setMetrics, setApprovals])

  // Periodically refresh metrics every 30s
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const metricsResponse = await metricsApi.getSystem()
        if (metricsResponse.data) {
          setMetrics(metricsResponse.data)
        }
      } catch {
        // Silently ignore polling errors
      }
    }, 30000)

    return () => clearInterval(interval)
  }, [setMetrics])
}
