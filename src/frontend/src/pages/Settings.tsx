import { useState, useEffect, useCallback } from 'react'
import { motion } from 'framer-motion'
import {
  Settings as SettingsIcon,
  Shield,
  Bell,
  Palette,
  Key,
  Save,
  Loader2,
} from 'lucide-react'
import { cn } from '../lib/utils'
import { settingsApi } from '../lib/api'
import toast from 'react-hot-toast'

type SettingsSection = 'general' | 'api' | 'limits' | 'notifications' | 'appearance'

const sections = [
  { id: 'general', label: 'General', icon: SettingsIcon },
  { id: 'api', label: 'API Keys', icon: Key },
  { id: 'limits', label: 'Resource Limits', icon: Shield },
  { id: 'notifications', label: 'Notifications', icon: Bell },
  { id: 'appearance', label: 'Appearance', icon: Palette },
] as const

export default function Settings() {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general')
  const [saving, setSaving] = useState(false)
  const [settings, setSettings] = useState({
    // General
    maxConcurrentAgents: 100,
    defaultModel: 'gpt-4o-mini',
    enableModelRouting: true,

    // API Keys
    openaiApiKey: '',
    anthropicApiKey: '',

    // Limits
    defaultTokenLimit: 20000,
    defaultCostLimit: 0.25,
    defaultTimeLimit: 300,
    circuitBreakerThreshold: 5,

    // Notifications
    notifyOnFailure: true,
    notifyOnCostThreshold: true,
    costThreshold: 1.0,

    // Appearance
    theme: 'dark',
    compactMode: false,
  })

  // Load settings from backend on mount
  const fetchSettings = useCallback(async () => {
    try {
      const response = await settingsApi.get()
      if (response.data) {
        const s = response.data
        setSettings((prev) => ({
          ...prev,
          maxConcurrentAgents: s.maxConcurrentTasks ?? prev.maxConcurrentAgents,
          defaultModel: s.defaultAgentModel ?? prev.defaultModel,
          defaultTokenLimit: prev.defaultTokenLimit,
          defaultCostLimit: prev.defaultCostLimit,
          defaultTimeLimit: prev.defaultTimeLimit,
          circuitBreakerThreshold: s.maxRetries ?? prev.circuitBreakerThreshold,
          enableModelRouting: prev.enableModelRouting,
          notifyOnFailure: prev.notifyOnFailure,
          notifyOnCostThreshold: prev.notifyOnCostThreshold,
          costThreshold: s.approvalThreshold ?? prev.costThreshold,
        }))
      }
    } catch {
      // Settings endpoint may not exist yet; use defaults
    }
  }, [])

  useEffect(() => {
    fetchSettings()
  }, [fetchSettings])

  const handleSave = async () => {
    setSaving(true)
    try {
      await settingsApi.update({
        maxConcurrentTasks: settings.maxConcurrentAgents,
        defaultAgentModel: settings.defaultModel,
        approvalThreshold: settings.costThreshold,
        autoRetryEnabled: true,
        maxRetries: settings.circuitBreakerThreshold,
        logLevel: 'info',
      })
      toast.success('Settings saved')
    } catch {
      toast.error('Failed to save settings')
    } finally {
      setSaving(false)
    }
  }

  const updateSetting = (key: string, value: unknown) => {
    setSettings((prev) => ({ ...prev, [key]: value }))
  }

  return (
    <div className="flex gap-6">
      {/* Sidebar */}
      <div className="w-64 flex-shrink-0">
        <nav className="space-y-1">
          {sections.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveSection(id as SettingsSection)}
              className={cn(
                'w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left transition-colors',
                activeSection === id
                  ? 'bg-apex-accent-primary/10 text-apex-accent-primary'
                  : 'text-apex-text-secondary hover:bg-apex-bg-tertiary'
              )}
            >
              <Icon size={20} />
              <span className="font-medium">{label}</span>
            </button>
          ))}
        </nav>
      </div>

      {/* Content */}
      <div className="flex-1 max-w-2xl">
        <motion.div
          key={activeSection}
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="space-y-6"
        >
          {activeSection === 'general' && (
            <>
              <div>
                <h2 className="text-xl font-semibold mb-1">General Settings</h2>
                <p className="text-apex-text-secondary">
                  Configure basic orchestrator behavior
                </p>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">
                    Max Concurrent Agents
                  </label>
                  <input
                    type="number"
                    value={settings.maxConcurrentAgents}
                    onChange={(e) =>
                      updateSetting('maxConcurrentAgents', parseInt(e.target.value))
                    }
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  />
                  <p className="text-xs text-apex-text-tertiary mt-1">
                    Maximum number of agents that can run simultaneously
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Default Model
                  </label>
                  <select
                    value={settings.defaultModel}
                    onChange={(e) => updateSetting('defaultModel', e.target.value)}
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  >
                    <option value="gpt-4o-mini">GPT-4o Mini</option>
                    <option value="gpt-4o">GPT-4o</option>
                    <option value="claude-3.5-haiku">Claude 3.5 Haiku</option>
                    <option value="claude-3.5-sonnet">Claude 3.5 Sonnet</option>
                  </select>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium">
                      Enable Model Routing
                    </label>
                    <p className="text-xs text-apex-text-tertiary">
                      Automatically select the most cost-effective model
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      updateSetting('enableModelRouting', !settings.enableModelRouting)
                    }
                    className={cn(
                      'w-12 h-6 rounded-full transition-colors',
                      settings.enableModelRouting
                        ? 'bg-apex-accent-primary'
                        : 'bg-apex-bg-tertiary'
                    )}
                  >
                    <div
                      className={cn(
                        'w-5 h-5 rounded-full bg-white transition-transform',
                        settings.enableModelRouting ? 'translate-x-6' : 'translate-x-0.5'
                      )}
                    />
                  </button>
                </div>
              </div>
            </>
          )}

          {activeSection === 'api' && (
            <>
              <div>
                <h2 className="text-xl font-semibold mb-1">API Keys</h2>
                <p className="text-apex-text-secondary">
                  Configure LLM provider credentials
                </p>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">
                    OpenAI API Key
                  </label>
                  <input
                    type="password"
                    value={settings.openaiApiKey}
                    onChange={(e) => updateSetting('openaiApiKey', e.target.value)}
                    placeholder="sk-..."
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary font-mono"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Anthropic API Key
                  </label>
                  <input
                    type="password"
                    value={settings.anthropicApiKey}
                    onChange={(e) => updateSetting('anthropicApiKey', e.target.value)}
                    placeholder="sk-ant-..."
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary font-mono"
                  />
                </div>
              </div>
            </>
          )}

          {activeSection === 'limits' && (
            <>
              <div>
                <h2 className="text-xl font-semibold mb-1">Resource Limits</h2>
                <p className="text-apex-text-secondary">
                  Set default resource constraints for tasks
                </p>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">
                    Default Token Limit
                  </label>
                  <input
                    type="number"
                    value={settings.defaultTokenLimit}
                    onChange={(e) =>
                      updateSetting('defaultTokenLimit', parseInt(e.target.value))
                    }
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Default Cost Limit ($)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={settings.defaultCostLimit}
                    onChange={(e) =>
                      updateSetting('defaultCostLimit', parseFloat(e.target.value))
                    }
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Default Time Limit (seconds)
                  </label>
                  <input
                    type="number"
                    value={settings.defaultTimeLimit}
                    onChange={(e) =>
                      updateSetting('defaultTimeLimit', parseInt(e.target.value))
                    }
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Circuit Breaker Threshold
                  </label>
                  <input
                    type="number"
                    value={settings.circuitBreakerThreshold}
                    onChange={(e) =>
                      updateSetting('circuitBreakerThreshold', parseInt(e.target.value))
                    }
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  />
                  <p className="text-xs text-apex-text-tertiary mt-1">
                    Number of consecutive failures before circuit breaks
                  </p>
                </div>
              </div>
            </>
          )}

          {activeSection === 'notifications' && (
            <>
              <div>
                <h2 className="text-xl font-semibold mb-1">Notifications</h2>
                <p className="text-apex-text-secondary">
                  Configure alerting preferences
                </p>
              </div>

              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium">
                      Notify on Task Failure
                    </label>
                    <p className="text-xs text-apex-text-tertiary">
                      Receive alerts when tasks fail
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      updateSetting('notifyOnFailure', !settings.notifyOnFailure)
                    }
                    className={cn(
                      'w-12 h-6 rounded-full transition-colors',
                      settings.notifyOnFailure
                        ? 'bg-apex-accent-primary'
                        : 'bg-apex-bg-tertiary'
                    )}
                  >
                    <div
                      className={cn(
                        'w-5 h-5 rounded-full bg-white transition-transform',
                        settings.notifyOnFailure ? 'translate-x-6' : 'translate-x-0.5'
                      )}
                    />
                  </button>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium">
                      Cost Threshold Alerts
                    </label>
                    <p className="text-xs text-apex-text-tertiary">
                      Alert when spending exceeds threshold
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      updateSetting(
                        'notifyOnCostThreshold',
                        !settings.notifyOnCostThreshold
                      )
                    }
                    className={cn(
                      'w-12 h-6 rounded-full transition-colors',
                      settings.notifyOnCostThreshold
                        ? 'bg-apex-accent-primary'
                        : 'bg-apex-bg-tertiary'
                    )}
                  >
                    <div
                      className={cn(
                        'w-5 h-5 rounded-full bg-white transition-transform',
                        settings.notifyOnCostThreshold
                          ? 'translate-x-6'
                          : 'translate-x-0.5'
                      )}
                    />
                  </button>
                </div>

                {settings.notifyOnCostThreshold && (
                  <div>
                    <label className="block text-sm font-medium mb-2">
                      Cost Threshold ($)
                    </label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.costThreshold}
                      onChange={(e) =>
                        updateSetting('costThreshold', parseFloat(e.target.value))
                      }
                      className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                    />
                  </div>
                )}
              </div>
            </>
          )}

          {activeSection === 'appearance' && (
            <>
              <div>
                <h2 className="text-xl font-semibold mb-1">Appearance</h2>
                <p className="text-apex-text-secondary">
                  Customize the dashboard look and feel
                </p>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">Theme</label>
                  <div className="flex gap-2">
                    {['dark', 'light', 'system'].map((theme) => (
                      <button
                        key={theme}
                        onClick={() => updateSetting('theme', theme)}
                        className={cn(
                          'px-4 py-2 rounded-lg capitalize transition-colors',
                          settings.theme === theme
                            ? 'bg-apex-accent-primary text-white'
                            : 'bg-apex-bg-secondary text-apex-text-secondary hover:bg-apex-bg-tertiary'
                        )}
                      >
                        {theme}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium">Compact Mode</label>
                    <p className="text-xs text-apex-text-tertiary">
                      Reduce spacing for more information density
                    </p>
                  </div>
                  <button
                    onClick={() => updateSetting('compactMode', !settings.compactMode)}
                    className={cn(
                      'w-12 h-6 rounded-full transition-colors',
                      settings.compactMode
                        ? 'bg-apex-accent-primary'
                        : 'bg-apex-bg-tertiary'
                    )}
                  >
                    <div
                      className={cn(
                        'w-5 h-5 rounded-full bg-white transition-transform',
                        settings.compactMode ? 'translate-x-6' : 'translate-x-0.5'
                      )}
                    />
                  </button>
                </div>
              </div>
            </>
          )}

          {/* Save Button */}
          <div className="pt-6 border-t border-apex-border-subtle">
            <button
              onClick={handleSave}
              disabled={saving}
              className="flex items-center gap-2 px-6 py-2 bg-apex-accent-primary hover:bg-blue-600 text-white rounded-lg transition-colors disabled:opacity-50"
            >
              {saving ? <Loader2 size={18} className="animate-spin" /> : <Save size={18} />}
              {saving ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </motion.div>
      </div>
    </div>
  )
}
