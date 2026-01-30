import { describe, it, expect, beforeEach } from 'vitest'
import { useApexStore } from '../lib/store'

describe('Apex Store', () => {
  beforeEach(() => {
    // Reset store before each test
    useApexStore.setState({
      agents: [],
      tasks: [],
      metrics: null,
      selectedAgentId: null,
      isConnected: false,
    })
  })

  describe('agents', () => {
    it('should start with empty agents array', () => {
      const { agents } = useApexStore.getState()
      expect(agents).toEqual([])
    })

    it('should set agents', () => {
      const mockAgents = [
        { id: 'agent-1', name: 'Agent 1', status: 'idle' as const },
        { id: 'agent-2', name: 'Agent 2', status: 'busy' as const },
      ]

      useApexStore.getState().setAgents(mockAgents as any)

      const { agents } = useApexStore.getState()
      expect(agents).toHaveLength(2)
      expect(agents[0].id).toBe('agent-1')
    })

    it('should update a single agent', () => {
      const mockAgents = [
        { id: 'agent-1', name: 'Agent 1', status: 'idle' as const },
      ]
      useApexStore.getState().setAgents(mockAgents as any)

      useApexStore.getState().updateAgent('agent-1', { status: 'busy' as const })

      const { agents } = useApexStore.getState()
      expect(agents[0].status).toBe('busy')
    })

    it('should select an agent', () => {
      useApexStore.getState().setSelectedAgentId('agent-1')

      const { selectedAgentId } = useApexStore.getState()
      expect(selectedAgentId).toBe('agent-1')
    })
  })

  describe('tasks', () => {
    it('should start with empty tasks array', () => {
      const { tasks } = useApexStore.getState()
      expect(tasks).toEqual([])
    })

    it('should set tasks', () => {
      const mockTasks = [
        { id: 'task-1', name: 'Task 1', status: 'pending' as const },
        { id: 'task-2', name: 'Task 2', status: 'running' as const },
      ]

      useApexStore.getState().setTasks(mockTasks as any)

      const { tasks } = useApexStore.getState()
      expect(tasks).toHaveLength(2)
    })

    it('should add a new task', () => {
      const newTask = { id: 'task-1', name: 'New Task', status: 'pending' as const }

      useApexStore.getState().addTask(newTask as any)

      const { tasks } = useApexStore.getState()
      expect(tasks).toHaveLength(1)
      expect(tasks[0].name).toBe('New Task')
    })

    it('should update a task', () => {
      const mockTasks = [
        { id: 'task-1', name: 'Task 1', status: 'pending' as const },
      ]
      useApexStore.getState().setTasks(mockTasks as any)

      useApexStore.getState().updateTask('task-1', { status: 'completed' as const })

      const { tasks } = useApexStore.getState()
      expect(tasks[0].status).toBe('completed')
    })
  })

  describe('connection', () => {
    it('should start disconnected', () => {
      const { isConnected } = useApexStore.getState()
      expect(isConnected).toBe(false)
    })

    it('should set connected state', () => {
      useApexStore.getState().setConnected(true)

      const { isConnected } = useApexStore.getState()
      expect(isConnected).toBe(true)
    })
  })

  describe('metrics', () => {
    it('should start with null metrics', () => {
      const { metrics } = useApexStore.getState()
      expect(metrics).toBeNull()
    })

    it('should set metrics', () => {
      const mockMetrics = {
        activeAgents: 10,
        pendingTasks: 25,
        completedToday: 150,
        costToday: 15.50,
      }

      useApexStore.getState().setMetrics(mockMetrics as any)

      const { metrics } = useApexStore.getState()
      expect(metrics?.activeAgents).toBe(10)
      expect(metrics?.costToday).toBe(15.50)
    })
  })
})
