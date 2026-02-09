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
  Plus,
  Trash2,
  Eye,
  EyeOff,
} from 'lucide-react'
import { cn } from '../lib/utils'
import { useStore } from '../lib/store'
import { settingsApi } from '../lib/api'
import toast from 'react-hot-toast'
import type { ApiKeySummary } from '../types'

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

  // Use Zustand store for settings and API keys
  const settings = useStore((s) => s.settings)
  const setSettings = useStore((s) => s.setSettings)
  const apiKeys = useStore((s) => s.apiKeys)
  const storeSetApiKeys = useStore((s) => s.setApiKeys)
  const storeAddApiKey = useStore((s) => s.addApiKey)
  const storeRemoveApiKey = useStore((s) => s.removeApiKey)

  // Local UI state for the add-key form
  const [loadingKeys, setLoadingKeys] = useState(false)
  const [showAddKey, setShowAddKey] = useState(false)
  const [newKeyProvider, setNewKeyProvider] = useState('')
  const [newKeyValue, setNewKeyValue] = useState('')
  const [showNewKeyValue, setShowNewKeyValue] = useState(false)
  const [addingKey, setAddingKey] = useState(false)

  // Load settings from backend on mount
  const fetchSettings = useCallback(async () => {
    try {
      const response = await settingsApi.get()
      if (response.data) {
        const s = response.data
        setSettings({
          maxConcurrentAgents: s.maxConcurrentTasks ?? settings.maxConcurrentAgents,
          defaultModel: s.defaultAgentModel ?? settings.defaultModel,
          autoRetryEnabled: s.autoRetryEnabled ?? settings.autoRetryEnabled,
          maxRetries: s.maxRetries ?? settings.maxRetries,
          logLevel: s.logLevel ?? settings.logLevel,
          costThreshold: s.approvalThreshold ?? settings.costThreshold,
        })
      }
    } catch {
      // Settings endpoint may not exist yet; use defaults
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const fetchApiKeys = useCallback(async () => {
    setLoadingKeys(true)
    try {
      const response = await settingsApi.getApiKeys()
      if (response.data) {
        storeSetApiKeys(response.data)
      }
    } catch {
      // API keys endpoint may not exist yet
    } finally {
      setLoadingKeys(false)
    }
  }, [storeSetApiKeys])

  useEffect(() => {
    fetchSettings()
    fetchApiKeys()
  }, [fetchSettings, fetchApiKeys])

  const handleSave = async () => {
    setSaving(true)
    try {
      await settingsApi.update({
        maxConcurrentTasks: settings.maxConcurrentAgents,
        defaultAgentModel: settings.defaultModel,
        approvalThreshold: settings.costThreshold,
        autoRetryEnabled: settings.autoRetryEnabled,
        maxRetries: settings.maxRetries,
        logLevel: settings.logLevel,
      })
      toast.success('Settings saved')
    } catch {
      toast.error('Failed to save settings')
    } finally {
      setSaving(false)
    }
  }

  const handleAddApiKey = async () => {
    if (!newKeyProvider.trim() || !newKeyValue.trim()) {
      toast.error('Provider and key are required')
      return
    }
    setAddingKey(true)
    try {
      const response = await settingsApi.createApiKey({
        provider: newKeyProvider.trim(),
        key: newKeyValue.trim(),
      })
      if (response.data) {
        storeAddApiKey(response.data)
        toast.success(`API key for ${newKeyProvider} added`)
        setNewKeyProvider('')
        setNewKeyValue('')
        setShowAddKey(false)
        setShowNewKeyValue(false)
      }
    } catch {
      toast.error('Failed to add API key')
    } finally {
      setAddingKey(false)
    }
  }

  const handleRevokeApiKey = async (key: ApiKeySummary) => {
    try {
      await settingsApi.revokeApiKey(key.id)
      storeRemoveApiKey(key.id)
      toast.success(`API key for ${key.provider} revoked`)
    } catch {
      toast.error('Failed to revoke API key')
    }
  }

  const updateSetting = (key: string, value: unknown) => {
    setSettings({ [key]: value })
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
                    <option value="claude-opus-4-6">Claude Opus 4.6</option>
                    <option value="claude-sonnet-4-5">Claude Sonnet 4.5</option>
                    <option value="claude-haiku-4-5">Claude Haiku 4.5</option>
                    <option value="gpt-5.2">GPT-5.2</option>
                    <option value="gpt-5.2-mini">GPT-5.2 Mini</option>
                    <option value="o3">o3</option>
                    <option value="o4-mini">o4-mini</option>
                    <option value="gemini-3-pro">Gemini 3 Pro</option>
                    <option value="gemini-3-flash">Gemini 3 Flash</option>
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

                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium">
                      Auto-Retry Failed Tasks
                    </label>
                    <p className="text-xs text-apex-text-tertiary">
                      Automatically retry tasks that fail
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      updateSetting('autoRetryEnabled', !settings.autoRetryEnabled)
                    }
                    className={cn(
                      'w-12 h-6 rounded-full transition-colors',
                      settings.autoRetryEnabled
                        ? 'bg-apex-accent-primary'
                        : 'bg-apex-bg-tertiary'
                    )}
                  >
                    <div
                      className={cn(
                        'w-5 h-5 rounded-full bg-white transition-transform',
                        settings.autoRetryEnabled ? 'translate-x-6' : 'translate-x-0.5'
                      )}
                    />
                  </button>
                </div>

                {settings.autoRetryEnabled && (
                  <div>
                    <label className="block text-sm font-medium mb-2">
                      Max Retries
                    </label>
                    <input
                      type="number"
                      min={0}
                      max={100}
                      value={settings.maxRetries}
                      onChange={(e) =>
                        updateSetting('maxRetries', parseInt(e.target.value) || 0)
                      }
                      className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                    />
                    <p className="text-xs text-apex-text-tertiary mt-1">
                      Maximum number of automatic retry attempts per task
                    </p>
                  </div>
                )}

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Log Level
                  </label>
                  <select
                    value={settings.logLevel}
                    onChange={(e) => updateSetting('logLevel', e.target.value)}
                    className="w-full px-4 py-2 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                  >
                    <option value="debug">Debug</option>
                    <option value="info">Info</option>
                    <option value="warn">Warning</option>
                    <option value="error">Error</option>
                  </select>
                  <p className="text-xs text-apex-text-tertiary mt-1">
                    Minimum log level for system output
                  </p>
                </div>
              </div>
            </>
          )}

          {activeSection === 'api' && (
            <>
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="text-xl font-semibold mb-1">API Keys</h2>
                  <p className="text-apex-text-secondary">
                    Manage LLM provider credentials
                  </p>
                </div>
                <button
                  onClick={() => setShowAddKey(!showAddKey)}
                  className="flex items-center gap-2 px-4 py-2 bg-apex-accent-primary hover:bg-blue-600 text-white rounded-lg transition-colors text-sm"
                >
                  <Plus size={16} />
                  Add Key
                </button>
              </div>

              {/* Add Key Form */}
              {showAddKey && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: 'auto' }}
                  className="p-4 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg space-y-3"
                >
                  <div>
                    <label className="block text-sm font-medium mb-1">Provider</label>
                    <select
                      value={newKeyProvider}
                      onChange={(e) => setNewKeyProvider(e.target.value)}
                      className="w-full px-4 py-2 bg-apex-bg-primary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary"
                    >
                      <option value="">Select provider...</option>
                      <option value="openai">OpenAI</option>
                      <option value="anthropic">Anthropic</option>
                      <option value="google">Google AI</option>
                      <option value="cohere">Cohere</option>
                      <option value="mistral">Mistral</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium mb-1">API Key</label>
                    <div className="relative">
                      <input
                        type={showNewKeyValue ? 'text' : 'password'}
                        value={newKeyValue}
                        onChange={(e) => setNewKeyValue(e.target.value)}
                        placeholder="sk-..."
                        className="w-full px-4 py-2 pr-10 bg-apex-bg-primary border border-apex-border-subtle rounded-lg focus:outline-none focus:border-apex-accent-primary font-mono"
                      />
                      <button
                        onClick={() => setShowNewKeyValue(!showNewKeyValue)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-apex-text-tertiary hover:text-apex-text-secondary"
                      >
                        {showNewKeyValue ? <EyeOff size={16} /> : <Eye size={16} />}
                      </button>
                    </div>
                  </div>
                  <div className="flex gap-2 justify-end">
                    <button
                      onClick={() => {
                        setShowAddKey(false)
                        setNewKeyProvider('')
                        setNewKeyValue('')
                      }}
                      className="px-4 py-2 text-sm text-apex-text-secondary hover:text-apex-text-primary transition-colors"
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleAddApiKey}
                      disabled={addingKey || !newKeyProvider || !newKeyValue}
                      className="flex items-center gap-2 px-4 py-2 bg-apex-accent-primary hover:bg-blue-600 text-white rounded-lg transition-colors text-sm disabled:opacity-50"
                    >
                      {addingKey && <Loader2 size={14} className="animate-spin" />}
                      Save Key
                    </button>
                  </div>
                </motion.div>
              )}

              {/* Keys List */}
              <div className="space-y-2">
                {loadingKeys ? (
                  <div className="flex items-center gap-2 text-apex-text-secondary py-8 justify-center">
                    <Loader2 size={18} className="animate-spin" />
                    Loading API keys...
                  </div>
                ) : apiKeys.length === 0 ? (
                  <div className="text-center py-8 text-apex-text-tertiary">
                    <Key size={32} className="mx-auto mb-2 opacity-40" />
                    <p>No API keys configured</p>
                    <p className="text-sm">Add a key to connect to LLM providers</p>
                  </div>
                ) : (
                  apiKeys.map((key) => (
                    <div
                      key={key.id}
                      className="flex items-center justify-between p-4 bg-apex-bg-secondary border border-apex-border-subtle rounded-lg"
                    >
                      <div className="flex items-center gap-3">
                        <div className="w-8 h-8 rounded-lg bg-apex-bg-tertiary flex items-center justify-center">
                          <Key size={16} className="text-apex-accent-primary" />
                        </div>
                        <div>
                          <p className="font-medium capitalize">{key.provider}</p>
                          <p className="text-sm text-apex-text-tertiary font-mono">
                            {key.maskedKey}
                          </p>
                        </div>
                      </div>
                      <div className="flex items-center gap-3">
                        <span className="text-xs text-apex-text-tertiary">
                          {new Date(key.createdAt).toLocaleDateString()}
                        </span>
                        <button
                          onClick={() => handleRevokeApiKey(key)}
                          className="p-2 text-apex-text-tertiary hover:text-red-400 transition-colors"
                          title="Revoke key"
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    </div>
                  ))
                )}
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

          {/* Save Button (not shown for API Keys section which has its own save) */}
          {activeSection !== 'api' && (
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
          )}
        </motion.div>
      </div>
    </div>
  )
}
