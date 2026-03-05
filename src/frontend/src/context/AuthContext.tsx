import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'
import { useStore } from '../lib/store'

interface AuthContextValue {
  isAuthenticated: boolean
  login: (username: string, password: string) => Promise<boolean>
  logout: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const { isAuthenticated, setIsAuthenticated } = useStore()

  const login = useCallback(async (username: string, password: string): Promise<boolean> => {
    try {
      const response = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      })

      if (response.ok) {
        setIsAuthenticated(true)
        return true
      }
      return false
    } catch {
      return false
    }
  }, [setIsAuthenticated])

  const logout = useCallback(() => {
    fetch('/api/auth/logout', { method: 'POST' }).finally(() => {
      setIsAuthenticated(false)
    })
  }, [setIsAuthenticated])

  return (
    <AuthContext.Provider value={{ isAuthenticated, login, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider')
  }
  return context
}
