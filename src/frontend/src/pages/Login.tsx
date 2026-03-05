import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useAuth } from '../context/AuthContext'
import toast from 'react-hot-toast'

export default function Login() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const { login } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()

  const from = (location.state as { from?: Location })?.from?.pathname || '/'

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!username.trim() || !password.trim()) {
      toast.error('Please enter username and password')
      return
    }

    setIsLoading(true)
    try {
      const success = await login(username, password)
      if (success) {
        toast.success('Welcome back!')
        navigate(from, { replace: true })
      } else {
        toast.error('Invalid credentials')
      }
    } catch {
      toast.error('Login failed. Please try again.')
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#0a0a0f]">
      <div className="w-full max-w-md p-8 rounded-2xl border border-[#2a2a3e] bg-[#12121a]/80 backdrop-blur-xl">
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-white mb-2">Apex</h1>
          <p className="text-[#8b8b9e] text-sm">Sign in to access the dashboard</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-6">
          <div>
            <label htmlFor="username" className="block text-xs font-medium text-[#8b8b9e] uppercase tracking-wider mb-2">
              Username
            </label>
            <input
              id="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full px-4 py-3 rounded-lg bg-[#1a1a2e] border border-[#2a2a3e] text-white placeholder-[#4a4a5e] focus:outline-none focus:border-blue-500/50 transition-colors"
              placeholder="Enter username"
              autoComplete="username"
            />
          </div>

          <div>
            <label htmlFor="password" className="block text-xs font-medium text-[#8b8b9e] uppercase tracking-wider mb-2">
              Password
            </label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-4 py-3 rounded-lg bg-[#1a1a2e] border border-[#2a2a3e] text-white placeholder-[#4a4a5e] focus:outline-none focus:border-blue-500/50 transition-colors"
              placeholder="Enter password"
              autoComplete="current-password"
            />
          </div>

          <button
            type="submit"
            disabled={isLoading}
            className="w-full py-3 px-4 rounded-lg bg-blue-500 hover:bg-blue-400 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isLoading ? 'Signing in...' : 'Sign In'}
          </button>
        </form>

        <p className="mt-6 text-center text-xs text-[#4a4a5e]">
          Protected by secure authentication
        </p>
      </div>
    </div>
  )
}
