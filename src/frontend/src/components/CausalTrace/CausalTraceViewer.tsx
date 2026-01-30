import { motion, AnimatePresence } from 'framer-motion'
import {
  Brain,
  Wrench,
  CheckCircle,
  AlertTriangle,
  ChevronDown,
  ExternalLink,
  Clock,
  Coins,
  Hash,
} from 'lucide-react'
import { useState } from 'react'
import { cn, formatCost, formatDuration } from '@/lib/utils'

// ---- Types ----

export type TraceStepType = 'llm_call' | 'tool_call' | 'result' | 'error'

export interface TraceStep {
  id: string
  type: TraceStepType
  label: string
  timestamp: string
  durationMs: number
  tokensUsed?: number
  costDollars?: number
  prompt?: string
  response?: string
  toolName?: string
  toolInput?: string
  toolOutput?: string
  errorMessage?: string
  jaegerTraceId?: string
  children?: TraceStep[]
}

interface CausalTraceViewerProps {
  steps: TraceStep[]
  jaegerBaseUrl?: string
}

// ---- Constants ----

const STEP_CONFIG: Record<
  TraceStepType,
  { color: string; bgColor: string; borderColor: string; icon: React.ReactNode; label: string }
> = {
  llm_call: {
    color: 'text-purple-400',
    bgColor: 'bg-purple-500/10',
    borderColor: 'border-purple-500/30',
    icon: <Brain size={14} />,
    label: 'LLM Call',
  },
  tool_call: {
    color: 'text-blue-400',
    bgColor: 'bg-blue-500/10',
    borderColor: 'border-blue-500/30',
    icon: <Wrench size={14} />,
    label: 'Tool Call',
  },
  result: {
    color: 'text-green-400',
    bgColor: 'bg-green-500/10',
    borderColor: 'border-green-500/30',
    icon: <CheckCircle size={14} />,
    label: 'Result',
  },
  error: {
    color: 'text-red-400',
    bgColor: 'bg-red-500/10',
    borderColor: 'border-red-500/30',
    icon: <AlertTriangle size={14} />,
    label: 'Error',
  },
}

// ---- Component ----

export function CausalTraceViewer({
  steps,
  jaegerBaseUrl = 'http://localhost:16686',
}: CausalTraceViewerProps) {
  if (steps.length === 0) {
    return (
      <div className="flex items-center justify-center h-64 text-apex-text-secondary">
        No trace data available
      </div>
    )
  }

  // Compute total cost and duration
  const totalCost = steps.reduce((acc, s) => acc + (s.costDollars ?? 0), 0)
  const totalDuration = steps.reduce((acc, s) => acc + s.durationMs, 0)
  const totalTokens = steps.reduce((acc, s) => acc + (s.tokensUsed ?? 0), 0)

  return (
    <div className="space-y-4">
      {/* Summary Bar */}
      <div className="flex items-center gap-6 px-4 py-2.5 bg-apex-bg-tertiary/50 rounded-lg border border-apex-border-subtle text-sm">
        <div className="flex items-center gap-1.5 text-apex-text-secondary">
          <Hash size={14} className="text-apex-text-tertiary" />
          <span>{steps.length} steps</span>
        </div>
        <div className="flex items-center gap-1.5 text-apex-text-secondary">
          <Clock size={14} className="text-apex-text-tertiary" />
          <span>{formatDuration(totalDuration)}</span>
        </div>
        <div className="flex items-center gap-1.5 text-apex-text-secondary">
          <Coins size={14} className="text-apex-text-tertiary" />
          <span>{formatCost(totalCost)}</span>
        </div>
        <div className="text-apex-text-tertiary text-xs">
          {totalTokens.toLocaleString()} tokens
        </div>
      </div>

      {/* Trace Tree */}
      <div className="relative">
        {steps.map((step, index) => (
          <TraceNode
            key={step.id}
            step={step}
            isLast={index === steps.length - 1}
            depth={0}
            jaegerBaseUrl={jaegerBaseUrl}
          />
        ))}
      </div>
    </div>
  )
}

// ---- TraceNode ----

function TraceNode({
  step,
  isLast,
  depth,
  jaegerBaseUrl,
}: {
  step: TraceStep
  isLast: boolean
  depth: number
  jaegerBaseUrl: string
}) {
  const [isExpanded, setIsExpanded] = useState(false)
  const config = STEP_CONFIG[step.type]

  return (
    <div className="relative">
      {/* Vertical connector line */}
      {!isLast && (
        <div
          className="absolute top-8 w-px bg-apex-border-subtle"
          style={{
            left: `${depth * 24 + 16}px`,
            height: 'calc(100% - 8px)',
          }}
        />
      )}

      {/* Node */}
      <div
        className="flex items-start gap-0"
        style={{ paddingLeft: `${depth * 24}px` }}
      >
        {/* Connector dot */}
        <div className="relative flex-shrink-0 w-8 flex items-center justify-center pt-3">
          <div
            className={cn(
              'w-3 h-3 rounded-full border-2 z-10',
              config.borderColor,
              config.bgColor
            )}
          />
        </div>

        {/* Card */}
        <motion.div
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          className="flex-1 mb-2"
        >
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className={cn(
              'w-full text-left px-3 py-2.5 rounded-lg border transition-all',
              'bg-apex-bg-secondary hover:bg-apex-bg-tertiary/70',
              isExpanded ? config.borderColor : 'border-apex-border-subtle'
            )}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <span className={config.color}>{config.icon}</span>
                <div>
                  <div className="flex items-center gap-2">
                    <span className={cn('text-xs font-medium', config.color)}>
                      {config.label}
                    </span>
                    <span className="text-sm font-medium text-apex-text-primary">
                      {step.label}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 mt-0.5 text-xs text-apex-text-tertiary">
                    <span>{formatDuration(step.durationMs)}</span>
                    {step.tokensUsed != null && step.tokensUsed > 0 && (
                      <span>{step.tokensUsed.toLocaleString()} tok</span>
                    )}
                    {step.costDollars != null && step.costDollars > 0 && (
                      <span>{formatCost(step.costDollars)}</span>
                    )}
                  </div>
                </div>
              </div>
              <ChevronDown
                size={14}
                className={cn(
                  'text-apex-text-tertiary transition-transform duration-200',
                  isExpanded && 'rotate-180'
                )}
              />
            </div>
          </button>

          {/* Expanded Details */}
          <AnimatePresence>
            {isExpanded && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2 }}
                className="overflow-hidden"
              >
                <div
                  className={cn(
                    'mt-1 px-3 py-3 rounded-lg border space-y-3',
                    'bg-apex-bg-primary',
                    config.borderColor
                  )}
                >
                  {/* Timestamp */}
                  <div>
                    <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                      Timestamp
                    </span>
                    <p className="text-xs font-mono text-apex-text-secondary mt-0.5">
                      {new Date(step.timestamp).toLocaleString()}
                    </p>
                  </div>

                  {/* LLM Call details */}
                  {step.type === 'llm_call' && (
                    <>
                      {step.prompt && (
                        <div>
                          <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                            Prompt
                          </span>
                          <pre className="mt-1 p-2 bg-apex-bg-secondary rounded text-xs font-mono text-apex-text-secondary whitespace-pre-wrap max-h-40 overflow-y-auto">
                            {step.prompt}
                          </pre>
                        </div>
                      )}
                      {step.response && (
                        <div>
                          <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                            Response
                          </span>
                          <pre className="mt-1 p-2 bg-apex-bg-secondary rounded text-xs font-mono text-apex-text-secondary whitespace-pre-wrap max-h-40 overflow-y-auto">
                            {step.response}
                          </pre>
                        </div>
                      )}
                    </>
                  )}

                  {/* Tool Call details */}
                  {step.type === 'tool_call' && (
                    <>
                      {step.toolName && (
                        <div>
                          <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                            Tool
                          </span>
                          <p className="text-xs font-mono text-blue-400 mt-0.5">
                            {step.toolName}
                          </p>
                        </div>
                      )}
                      {step.toolInput && (
                        <div>
                          <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                            Input
                          </span>
                          <pre className="mt-1 p-2 bg-apex-bg-secondary rounded text-xs font-mono text-apex-text-secondary whitespace-pre-wrap max-h-32 overflow-y-auto">
                            {step.toolInput}
                          </pre>
                        </div>
                      )}
                      {step.toolOutput && (
                        <div>
                          <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                            Output
                          </span>
                          <pre className="mt-1 p-2 bg-apex-bg-secondary rounded text-xs font-mono text-apex-text-secondary whitespace-pre-wrap max-h-32 overflow-y-auto">
                            {step.toolOutput}
                          </pre>
                        </div>
                      )}
                    </>
                  )}

                  {/* Error details */}
                  {step.type === 'error' && step.errorMessage && (
                    <div>
                      <span className="text-xxs font-medium text-red-400 uppercase tracking-wider">
                        Error
                      </span>
                      <pre className="mt-1 p-2 bg-red-500/5 border border-red-500/20 rounded text-xs font-mono text-red-400 whitespace-pre-wrap max-h-32 overflow-y-auto">
                        {step.errorMessage}
                      </pre>
                    </div>
                  )}

                  {/* Result details */}
                  {step.type === 'result' && step.response && (
                    <div>
                      <span className="text-xxs font-medium text-apex-text-tertiary uppercase tracking-wider">
                        Result
                      </span>
                      <pre className="mt-1 p-2 bg-apex-bg-secondary rounded text-xs font-mono text-apex-text-secondary whitespace-pre-wrap max-h-40 overflow-y-auto">
                        {step.response}
                      </pre>
                    </div>
                  )}

                  {/* Jaeger link */}
                  {step.jaegerTraceId && (
                    <a
                      href={`${jaegerBaseUrl}/trace/${step.jaegerTraceId}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1.5 text-xs text-apex-accent-primary hover:underline"
                    >
                      <ExternalLink size={12} />
                      View in Jaeger
                    </a>
                  )}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>
      </div>

      {/* Render children */}
      {step.children?.map((child, index) => (
        <TraceNode
          key={child.id}
          step={child}
          isLast={index === (step.children?.length ?? 0) - 1}
          depth={depth + 1}
          jaegerBaseUrl={jaegerBaseUrl}
        />
      ))}
    </div>
  )
}
