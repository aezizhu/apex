import { Routes, Route, Navigate } from 'react-router-dom'
import { Toaster } from 'react-hot-toast'
import { AuthProvider } from './context/AuthContext'
import { ProtectedRoute } from './components/ProtectedRoute'
import { useWebSocket } from './hooks/useWebSocket'
import { useInitialData } from './hooks/useInitialData'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import Agents from './pages/Agents'
import Tasks from './pages/Tasks'
import Approvals from './pages/Approvals'
import Settings from './pages/Settings'
import AgentSightPage from './pages/AgentSight'
import Workflows from './pages/Workflows'
import Login from './pages/Login'

function App() {
  // Connect to WebSocket for real-time updates
  useWebSocket()
  // Fetch initial data from REST API and set up periodic polling
  useInitialData()

  return (
    <AuthProvider>
      <Layout>
        <Routes>
          {/* Public routes */}
          <Route path="/login" element={<Login />} />

          {/* Protected routes */}
          <Route
            path="/"
            element={
              <ProtectedRoute>
                <Dashboard />
              </ProtectedRoute>
            }
          />
          <Route
            path="/agents"
            element={
              <ProtectedRoute>
                <Agents />
              </ProtectedRoute>
            }
          />
          <Route
            path="/tasks"
            element={
              <ProtectedRoute>
                <Tasks />
              </ProtectedRoute>
            }
          />
          <Route
            path="/approvals"
            element={
              <ProtectedRoute>
                <Approvals />
              </ProtectedRoute>
            }
          />
          <Route
            path="/agent-sight"
            element={
              <ProtectedRoute>
                <AgentSightPage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/workflows"
            element={
              <ProtectedRoute>
                <Workflows />
              </ProtectedRoute>
            }
          />
          <Route
            path="/settings"
            element={
              <ProtectedRoute>
                <Settings />
              </ProtectedRoute>
            }
          />

          {/* Redirect root to dashboard */}
          <Route path="*" element={<Navigate to="/" replace />} />
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
    </AuthProvider>
  )
}

export default App
