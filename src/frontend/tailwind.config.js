/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        // Apex Dark Theme
        apex: {
          bg: {
            primary: '#0a0a0f',
            secondary: '#12121a',
            tertiary: '#1a1a2e',
            elevated: '#242438',
          },
          border: {
            subtle: '#2a2a3e',
            default: '#3a3a4e',
            strong: '#4a4a5e',
          },
          text: {
            primary: '#f8fafc',
            secondary: '#94a3b8',
            tertiary: '#64748b',
            muted: '#475569',
          },
          // Status colors
          status: {
            busy: '#3b82f6',
            idle: '#6b7280',
            error: '#ef4444',
            waiting: '#f59e0b',
            success: '#10b981',
            paused: '#8b5cf6',
          },
          // Confidence heatmap
          confidence: {
            high: '#1e40af',
            medium: '#3b82f6',
            low: '#ef4444',
          },
          // Accent colors
          accent: {
            primary: '#3b82f6',
            secondary: '#8b5cf6',
            tertiary: '#06b6d4',
          },
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Menlo', 'monospace'],
      },
      fontSize: {
        'xxs': '0.625rem',
      },
      spacing: {
        '18': '4.5rem',
        '88': '22rem',
        '128': '32rem',
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'spin-slow': 'spin 2s linear infinite',
        'glow': 'glow 2s ease-in-out infinite alternate',
        'slide-in': 'slideIn 0.2s ease-out',
        'fade-in': 'fadeIn 0.15s ease-out',
      },
      keyframes: {
        glow: {
          '0%': { boxShadow: '0 0 5px rgba(59, 130, 246, 0.5)' },
          '100%': { boxShadow: '0 0 20px rgba(59, 130, 246, 0.8)' },
        },
        slideIn: {
          '0%': { transform: 'translateY(-10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
      },
      boxShadow: {
        'glow-sm': '0 0 10px rgba(59, 130, 246, 0.3)',
        'glow-md': '0 0 20px rgba(59, 130, 246, 0.4)',
        'glow-lg': '0 0 30px rgba(59, 130, 246, 0.5)',
        'inner-glow': 'inset 0 0 20px rgba(59, 130, 246, 0.1)',
      },
      backdropBlur: {
        xs: '2px',
      },
    },
  },
  plugins: [],
}
