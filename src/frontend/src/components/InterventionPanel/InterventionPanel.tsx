import * as AlertDialog from '@radix-ui/react-alert-dialog'
import { motion, AnimatePresence } from 'framer-motion'
import {
  X,
  Send,
  Pause,
  Play,
  Shield,
  AlertOctagon,
  Terminal,
  Keyboard,
  ChevronRight,
} from 'lucide-react'
import { useState, useCallback, useEffect } from 'react'
import { Button } from '@/components/ui/Button'
import { Textarea } from '@/components/ui/Input'
import { type Agent } from '@/lib/store'
import { cn } from '@/lib/utils'

interface InterventionPanelProps {
  agent: Agent
  onClose: () => void
  onSendMessage?: (agentId: string, message: string) => void
  onPause?: (agentId: string) => void
  onResume?: (agentId: string) => void
  onPatchState?: (agentId: string, state: string) => void
  onTakeover?: (agentId: string) => void
  onKill?: (agentId: string) => void
}

type Section = 'nudge' | 'pause' | 'takeover' | 'kill'

export function InterventionPanel({
  agent,
  onClose,
  onSendMessage,
  onPause,
  onResume,
  onPatchState,
  onTakeover,
  onKill,
}: InterventionPanelProps) {
  const [expandedSection, setExpandedSection] = useState<Section | null>('nudge')
  const [nudgeMessage, setNudgeMessage] = useState('')
  const [isPaused, setIsPaused] = useState(agent.status === 'paused')
  const [agentState, setAgentState] = useState(
    JSON.stringify(
      {
        id: agent.id,
        model: agent.model,
        currentLoad: agent.currentLoad,
        maxLoad: agent.maxLoad,
        temperature: 0.7,
        maxTokens: 4096,
      },
      null,
      2
    )
  )
  const [isTakeover, setIsTakeover] = useState(false)

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Only handle when not typing in an input
      if (
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLInputElement
      ) {
        // Allow Cmd/Ctrl+Enter to send nudge
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && expandedSection === 'nudge') {
          e.preventDefault()
          handleSendNudge()
        }
        return
      }

      switch (e.key) {
        case 'Escape':
          onClose()
          break
        case '1':
          setExpandedSection(expandedSection === 'nudge' ? null : 'nudge')
          break
        case '2':
          setExpandedSection(expandedSection === 'pause' ? null : 'pause')
          break
        case '3':
          setExpandedSection(expandedSection === 'takeover' ? null : 'takeover')
          break
        case '4':
          setExpandedSection(expandedSection === 'kill' ? null : 'kill')
          break
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expandedSection, onClose])

  const handleSendNudge = useCallback(() => {
    if (!nudgeMessage.trim()) return
    onSendMessage?.(agent.id, nudgeMessage)
    setNudgeMessage('')
  }, [agent.id, nudgeMessage, onSendMessage])

  const handlePauseToggle = useCallback(() => {
    if (isPaused) {
      onResume?.(agent.id)
    } else {
      onPause?.(agent.id)
    }
    setIsPaused(!isPaused)
  }, [agent.id, isPaused, onPause, onResume])

  const handlePatchState = useCallback(() => {
    onPatchState?.(agent.id, agentState)
  }, [agent.id, agentState, onPatchState])

  const handleTakeover = useCallback(() => {
    onTakeover?.(agent.id)
    setIsTakeover(true)
  }, [agent.id, onTakeover])

  const toggleSection = (section: Section) => {
    setExpandedSection(expandedSection === section ? null : section)
  }

  const sections: Array<{
    id: Section
    title: string
    icon: React.ReactNode
    shortcut: string
    color: string
  }> = [
    {
      id: 'nudge',
      title: 'Nudge',
      icon: <Send size={16} />,
      shortcut: '1',
      color: 'text-blue-400',
    },
    {
      id: 'pause',
      title: 'Pause & Patch',
      icon: <Pause size={16} />,
      shortcut: '2',
      color: 'text-yellow-400',
    },
    {
      id: 'takeover',
      title: 'Takeover',
      icon: <Terminal size={16} />,
      shortcut: '3',
      color: 'text-purple-400',
    },
    {
      id: 'kill',
      title: 'Kill Switch',
      icon: <AlertOctagon size={16} />,
      shortcut: '4',
      color: 'text-red-400',
    },
  ]

  return (
    <motion.div
      initial={{ x: '100%' }}
      animate={{ x: 0 }}
      exit={{ x: '100%' }}
      transition={{ type: 'spring', damping: 25, stiffness: 250 }}
      className="fixed right-0 top-0 bottom-0 w-[420px] bg-apex-bg-secondary border-l border-apex-border-subtle shadow-2xl z-50 flex flex-col"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-apex-border-subtle">
        <div className="flex items-center gap-3">
          <Shield size={20} className="text-apex-accent-primary" />
          <div>
            <h2 className="font-semibold text-apex-text-primary">
              Intervention
            </h2>
            <p className="text-xs text-apex-text-tertiary font-mono">
              {agent.name} ({agent.id.slice(0, 8)})
            </p>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1.5 hover:bg-apex-bg-tertiary rounded-md transition-colors"
        >
          <X size={18} className="text-apex-text-tertiary" />
        </button>
      </div>

      {/* Agent Status Bar */}
      <div className="px-5 py-3 bg-apex-bg-tertiary/50 border-b border-apex-border-subtle">
        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2">
            <div
              className={cn(
                'w-2 h-2 rounded-full',
                agent.status === 'busy' && 'bg-blue-500 animate-pulse',
                agent.status === 'idle' && 'bg-gray-500',
                agent.status === 'error' && 'bg-red-500',
                agent.status === 'paused' && 'bg-yellow-500'
              )}
            />
            <span className="text-apex-text-secondary capitalize">
              {isPaused ? 'paused' : agent.status}
            </span>
          </div>
          <div className="flex items-center gap-1 text-apex-text-tertiary text-xs">
            <Keyboard size={12} />
            <span>Esc to close</span>
          </div>
        </div>
      </div>

      {/* Sections */}
      <div className="flex-1 overflow-y-auto">
        {sections.map((section) => (
          <div
            key={section.id}
            className="border-b border-apex-border-subtle"
          >
            {/* Section Header */}
            <button
              onClick={() => toggleSection(section.id)}
              className="w-full flex items-center justify-between px-5 py-3 hover:bg-apex-bg-tertiary/50 transition-colors"
            >
              <div className="flex items-center gap-3">
                <span className={section.color}>{section.icon}</span>
                <span className="font-medium text-sm text-apex-text-primary">
                  {section.title}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xxs text-apex-text-muted bg-apex-bg-tertiary px-1.5 py-0.5 rounded font-mono">
                  {section.shortcut}
                </span>
                <ChevronRight
                  size={14}
                  className={cn(
                    'text-apex-text-tertiary transition-transform duration-200',
                    expandedSection === section.id && 'rotate-90'
                  )}
                />
              </div>
            </button>

            {/* Section Content */}
            <AnimatePresence>
              {expandedSection === section.id && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.2 }}
                  className="overflow-hidden"
                >
                  <div className="px-5 pb-4 space-y-3">
                    {section.id === 'nudge' && (
                      <NudgeSection
                        message={nudgeMessage}
                        onMessageChange={setNudgeMessage}
                        onSend={handleSendNudge}
                      />
                    )}
                    {section.id === 'pause' && (
                      <PauseSection
                        isPaused={isPaused}
                        agentState={agentState}
                        onTogglePause={handlePauseToggle}
                        onStateChange={setAgentState}
                        onApplyPatch={handlePatchState}
                      />
                    )}
                    {section.id === 'takeover' && (
                      <TakeoverSection
                        isTakeover={isTakeover}
                        agentName={agent.name}
                        onTakeover={handleTakeover}
                      />
                    )}
                    {section.id === 'kill' && (
                      <KillSection
                        agentName={agent.name}
                        agentId={agent.id}
                        onKill={onKill}
                      />
                    )}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        ))}
      </div>
    </motion.div>
  )
}

// ---- Sub-sections ----

function NudgeSection({
  message,
  onMessageChange,
  onSend,
}: {
  message: string
  onMessageChange: (msg: string) => void
  onSend: () => void
}) {
  return (
    <>
      <p className="text-xs text-apex-text-tertiary">
        Send a system-level message to nudge the agent's behavior.
      </p>
      <Textarea
        placeholder="e.g., Focus on completing the current subtask before moving on..."
        value={message}
        onChange={(e) => onMessageChange(e.target.value)}
        rows={3}
        className="text-sm"
      />
      <div className="flex items-center justify-between">
        <span className="text-xxs text-apex-text-muted">
          <kbd className="px-1 py-0.5 bg-apex-bg-tertiary rounded text-xxs font-mono">
            Cmd+Enter
          </kbd>{' '}
          to send
        </span>
        <Button
          size="sm"
          onClick={onSend}
          disabled={!message.trim()}
          leftIcon={<Send size={14} />}
        >
          Send Message
        </Button>
      </div>
    </>
  )
}

function PauseSection({
  isPaused,
  agentState,
  onTogglePause,
  onStateChange,
  onApplyPatch,
}: {
  isPaused: boolean
  agentState: string
  onTogglePause: () => void
  onStateChange: (state: string) => void
  onApplyPatch: () => void
}) {
  const [isValidJson, setIsValidJson] = useState(true)

  const handleStateChange = (value: string) => {
    onStateChange(value)
    try {
      JSON.parse(value)
      setIsValidJson(true)
    } catch {
      setIsValidJson(false)
    }
  }

  return (
    <>
      <p className="text-xs text-apex-text-tertiary">
        {isPaused
          ? 'Agent is paused. Edit state below and resume.'
          : 'Pause the agent to inspect and modify its state.'}
      </p>
      <Button
        size="sm"
        variant={isPaused ? 'success' : 'secondary'}
        onClick={onTogglePause}
        leftIcon={isPaused ? <Play size={14} /> : <Pause size={14} />}
        className="w-full"
      >
        {isPaused ? 'Resume Agent' : 'Pause Agent'}
      </Button>

      {isPaused && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          className="space-y-2"
        >
          <label className="text-xs font-medium text-apex-text-secondary">
            Agent State (JSON)
          </label>
          <textarea
            value={agentState}
            onChange={(e) => handleStateChange(e.target.value)}
            rows={8}
            spellCheck={false}
            className={cn(
              'w-full px-3 py-2 bg-apex-bg-primary border rounded-lg font-mono text-xs text-apex-text-primary focus:outline-none focus:ring-1 resize-none',
              isValidJson
                ? 'border-apex-border-subtle focus:ring-apex-accent-primary focus:border-apex-accent-primary'
                : 'border-red-500 focus:ring-red-500 focus:border-red-500'
            )}
          />
          {!isValidJson && (
            <p className="text-xs text-red-500">Invalid JSON</p>
          )}
          <Button
            size="sm"
            variant="primary"
            onClick={onApplyPatch}
            disabled={!isValidJson}
            className="w-full"
          >
            Apply State Patch
          </Button>
        </motion.div>
      )}
    </>
  )
}

function TakeoverSection({
  isTakeover,
  agentName,
  onTakeover,
}: {
  isTakeover: boolean
  agentName: string
  onTakeover: () => void
}) {
  return (
    <>
      <p className="text-xs text-apex-text-tertiary">
        Take direct control of {agentName}'s execution context and conversation.
      </p>
      {!isTakeover ? (
        <Button
          size="sm"
          variant="secondary"
          onClick={onTakeover}
          leftIcon={<Terminal size={14} />}
          className="w-full"
        >
          Take Control
        </Button>
      ) : (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="space-y-3"
        >
          <div className="bg-apex-bg-primary border border-apex-border-subtle rounded-lg p-3">
            <div className="flex items-center gap-2 mb-2">
              <Terminal size={14} className="text-purple-400" />
              <span className="text-xs font-medium text-purple-400">
                Live Context
              </span>
            </div>
            <div className="font-mono text-xs text-apex-text-secondary space-y-1">
              <p className="text-apex-text-tertiary">
                {'>'} System: You are an AI assistant...
              </p>
              <p className="text-apex-text-tertiary">
                {'>'} User: Process the following data...
              </p>
              <p className="text-apex-text-primary">
                {'>'} Assistant: I'll analyze the data by...
              </p>
            </div>
          </div>
          <p className="text-xxs text-yellow-500">
            You have control. The agent is waiting for your input.
          </p>
        </motion.div>
      )}
    </>
  )
}

function KillSection({
  agentName,
  agentId,
  onKill,
}: {
  agentName: string
  agentId: string
  onKill?: (agentId: string) => void
}) {
  return (
    <>
      <p className="text-xs text-apex-text-tertiary">
        Immediately terminate all processes for {agentName}. This action cannot
        be undone.
      </p>
      <AlertDialog.Root>
        <AlertDialog.Trigger asChild>
          <Button
            size="sm"
            variant="danger"
            leftIcon={<AlertOctagon size={14} />}
            className="w-full"
          >
            Emergency Stop
          </Button>
        </AlertDialog.Trigger>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/60 backdrop-blur-sm z-[60]" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-apex-bg-secondary border border-apex-border-default rounded-xl p-6 max-w-md w-full z-[61] shadow-2xl">
            <AlertDialog.Title className="text-lg font-semibold text-apex-text-primary">
              Confirm Emergency Stop
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-apex-text-secondary">
              This will immediately terminate <strong>{agentName}</strong> (
              {agentId.slice(0, 8)}). All in-progress tasks will be cancelled
              and any unsaved state will be lost.
            </AlertDialog.Description>
            <div className="flex justify-end gap-3 mt-6">
              <AlertDialog.Cancel asChild>
                <Button variant="secondary" size="sm">
                  Cancel
                </Button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => onKill?.(agentId)}
                >
                  Kill Agent
                </Button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </>
  )
}
