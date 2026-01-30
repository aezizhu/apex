import { Routes, Route } from 'react-router-dom'
import { Toaster } from 'react-hot-toast'
import { useWebSocket } from './hooks/useWebSocket'
import { useInitialData } from './hooks/useInitialData'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import Agents from './pages/Agents'
import Tasks from './pages/Tasks'
import Approvals from './pages/Approvals'
import Settings from './pages/Settings'
import AgentSightPage from './pages/AgentSight'

function App() {
  // Connect to WebSocket for real-time updates
  useWebSocket()
  // Fetch initial data from REST API and set up periodic polling
  useInitialData()

  return (
    <>
      <Layout>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/agents" element={<Agents />} />
          <Route path="/tasks" element={<Tasks />} />
          <Route path="/approvals" element={<Approvals />} />
          <Route path="/agent-sight" element={<AgentSightPage />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </Layout>
      <Toaster
        position="bottom-right"
        toastOptions={{
          className: 'glass',
          style: {
            background: '#1a1a2e',
            color: '#f8fafc',
            border: '1px solid #2a2a3e',
          },
        }}
      />
    </>
  )
}

export default App
