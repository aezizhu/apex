# Panopticon Protocol - Frontend Design Document

> **Project Apex** | Command Center Dashboard for Multi-Agent AI Orchestration
> **Version:** 1.0.0
> **Last Updated:** 2026-01-29

---

## Table of Contents

1. [Design Philosophy](#1-design-philosophy)
2. [Core Components Design](#2-core-components-design)
3. [Technology Stack](#3-technology-stack)
4. [Design System](#4-design-system)
5. [Component Architecture](#5-component-architecture)
6. [Wireframes](#6-wireframes)
7. [Accessibility (WCAG 2.1 AA)](#7-accessibility-wcag-21-aa)
8. [Performance Targets](#8-performance-targets)

---

## 1. Design Philosophy

The Panopticon Protocol dashboard embodies a **command center aesthetic** - a mission-critical interface designed for operators managing large-scale AI agent deployments. Every design decision prioritizes clarity, responsiveness, and operator confidence.

### 1.1 Core Principles

#### Command Center Aesthetic (Dark Mode Primary)

The interface draws inspiration from aerospace mission control and financial trading terminals:

- **Dark-first design** reduces eye strain during extended monitoring sessions
- **High-contrast elements** ensure critical information stands out
- **Ambient lighting effects** provide subtle environmental feedback without distraction
- **Professional, serious tone** reinforces the gravity of overseeing autonomous AI systems

#### Information Density Without Clutter

Operators need comprehensive situational awareness while avoiding cognitive overload:

- **Progressive disclosure** - Summary views with drill-down capability
- **Contextual grouping** - Related metrics positioned together
- **Visual hierarchy** - Size, color, and position indicate importance
- **Negative space** - Strategic use of empty space prevents overwhelming displays
- **Information layering** - Base data visible; overlays reveal deeper insights

#### Real-Time Updates with Sub-100ms Latency Feel

The dashboard must feel instantaneous to maintain operator trust:

- **Optimistic UI updates** - Reflect expected changes before server confirmation
- **Smooth animations** - Transitions at 60fps prevent jarring updates
- **Streaming data** - WebSocket connections eliminate polling delays
- **Predictive rendering** - Anticipate likely user actions and preload data
- **Visual continuity** - Animated transitions show data evolution, not replacement

#### Accessible and Keyboard-Navigable

Mission-critical systems must be operable by all users in all conditions:

- **Full keyboard navigation** - Every action accessible without a mouse
- **Screen reader compatibility** - ARIA labels and semantic HTML throughout
- **Color-independent indicators** - Icons and patterns supplement color coding
- **Customizable contrast** - Support for high-contrast mode
- **Focus management** - Clear, visible focus indicators at all times

### 1.2 Emotional Design Goals

| Emotion | How We Achieve It |
|---------|-------------------|
| **Confidence** | Clear data, consistent patterns, reliable updates |
| **Control** | Immediate feedback, undo capabilities, clear action paths |
| **Awareness** | Comprehensive views, proactive alerts, trend visibility |
| **Calm** | Subdued colors for normal states, reserved intensity for alerts |

---

## 2. Core Components Design

### 2.a Agent Hex Grid Visualization

The heart of the Panopticon Protocol - a real-time visualization of all active agents displayed in an efficient hexagonal grid pattern.

#### Layout Rationale

Hexagonal grids provide:
- **15% higher packing efficiency** than square grids
- **Equidistant neighbors** - each cell has 6 equally-spaced adjacent cells
- **Organic appearance** - reduces the clinical feel of a surveillance interface
- **Natural clustering** - groups of agents visually cohere

#### Visual States

```
Agent State Colors:
┌─────────────────────────────────────────────────────────┐
│  State      │  Color     │  Hex Code   │  Pulse Effect │
├─────────────────────────────────────────────────────────┤
│  Busy       │  Blue      │  #3b82f6    │  Gentle pulse │
│  Idle       │  Gray      │  #4b5563    │  None         │
│  Error      │  Red       │  #ef4444    │  Rapid pulse  │
│  Waiting    │  Orange    │  #f59e0b    │  Slow pulse   │
│  Success    │  Green     │  #10b981    │  Brief flash  │
│  Paused     │  Purple    │  #8b5cf6    │  Static glow  │
└─────────────────────────────────────────────────────────┘
```

#### Confidence Heatmap Overlay

A toggleable overlay that visualizes agent confidence levels:

- **Gradient mapping**: 0% (deep red) -> 50% (yellow) -> 100% (green)
- **Opacity control**: Slider to adjust overlay intensity (0-100%)
- **Threshold filtering**: Show only agents below/above confidence threshold
- **Temporal smoothing**: 3-second rolling average prevents flicker

#### Hover Card Design

```
┌────────────────────────────────────────┐
│  Agent: orchestrator-7f3a              │
│  ────────────────────────────────────  │
│  Task: Analyzing customer feedback     │
│  Progress: ████████░░ 78%              │
│  ────────────────────────────────────  │
│  Tokens: 12,847 / 32,000               │
│  Cost: $0.0342                         │
│  Success Rate: 94.2%                   │
│  Uptime: 2h 34m                        │
│  ────────────────────────────────────  │
│  [View Details]  [Intervene]           │
└────────────────────────────────────────┘
```

#### Navigation Controls

- **Zoom**: Mouse wheel / pinch gesture / +/- buttons (10 levels: 10% - 400%)
- **Pan**: Click-drag / arrow keys / WASD
- **Reset**: Double-click / Home key returns to fit-all view
- **Minimap**: 150x100px overview in bottom-right corner
  - Current viewport shown as semi-transparent rectangle
  - Click-to-navigate functionality
  - Collapsible via toggle button

#### Interaction Patterns

| Action | Mouse | Keyboard | Touch |
|--------|-------|----------|-------|
| Select agent | Click | Enter (on focused) | Tap |
| Multi-select | Ctrl+Click | Shift+Arrow | Long press + drag |
| Pan view | Drag background | Arrow keys | Two-finger drag |
| Zoom | Scroll wheel | +/- keys | Pinch |
| Open details | Double-click | Space | Double-tap |
| Context menu | Right-click | Menu key | Long press |

---

### 2.b Critical Path Timeline

A Gantt-style visualization showing task dependencies, scheduling, and the critical path through the agent workflow.

#### Visual Design

```
Timeline Header:
┌──────────────────────────────────────────────────────────────────┐
│  ◀ │ 2026-01-29 │ ▶ │  [Hour] [Day] [Week]  │  🔍 Search Tasks  │
└──────────────────────────────────────────────────────────────────┘

Task Bars:
─────┬────────────────────────────────────────────────────────────
     │    00:00    04:00    08:00    12:00    16:00    20:00
─────┼────────────────────────────────────────────────────────────
Task │    ████████████████▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
 A   │    [Completed]     [In Progress]
─────┼────────────────────────────────────────────────────────────
Task │                    ░░░░░░░░████████████████████████▓▓▓▓▓▓
 B   │                    [Waiting]  [Critical Path - Highlighted]
─────┼────────────────────────────────────────────────────────────
Task │    ████████████████████████████████████░░░░░░░░░░░░░░░░░░
 C   │    [Parallel Track - Dimmed]
─────┴────────────────────────────────────────────────────────────

Legend:
████ Completed    ▓▓▓▓ In Progress    ░░░░ Scheduled    ╳╳╳╳ Blocked
```

#### Critical Path Highlighting

- **Bold red border** around critical path tasks
- **Dependency lines** drawn between connected tasks (Bezier curves)
- **Slack visualization** - lighter colored extensions showing available slack time
- **Bottleneck indicators** - diamond markers on constrained resources

#### Interactive Features

| Feature | Description |
|---------|-------------|
| **Drag to reschedule** | Grab task bar to move within constraints |
| **Resize duration** | Drag task edges to adjust estimated time |
| **Dependency editing** | Ctrl+drag between tasks to create/modify links |
| **Time scrubber** | Slider control for historical playback |
| **Playback speed** | 1x, 2x, 4x, 8x for historical review |
| **Snapshot markers** | Bookmarkable points in timeline |

#### Time Scrubber Controls

```
┌─────────────────────────────────────────────────────────────┐
│  ◀◀  │  ◀  │  ▶  │  ▶▶  │  ──●───────────────────  │ 1x ▼ │
│  -1h   -5m   +5m   +1h      08:00            Now           │
└─────────────────────────────────────────────────────────────┘
```

---

### 2.c Approval Queue Dashboard

A triage interface for managing high-impact actions requiring human authorization.

#### Queue Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│  APPROVAL QUEUE                              Pending: 23  │ ⚙ Filter │
├─────────────────────────────────────────────────────────────────────┤
│  [Semantic Clusters]                                                │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐       │
│  │ Database Writes │ │ API Calls       │ │ File Operations │       │
│  │      (8)        │ │      (12)       │ │      (3)        │       │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘       │
├─────────────────────────────────────────────────────────────────────┤
│  ► CRITICAL (3)                                                     │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ ● agent-db-writer wants to DROP TABLE users                     ││
│  │   Risk: CRITICAL │ Confidence: 34% │ Requested: 2m ago         ││
│  │   Similar to 2 other requests                                   ││
│  │   [A] Approve  [D] Deny  [V] View Details  [G] Group Actions   ││
│  └─────────────────────────────────────────────────────────────────┘│
│  ► HIGH (7)                                                         │
│  ► MEDIUM (8)                                                       │
│  ► LOW (5)                                                          │
└─────────────────────────────────────────────────────────────────────┘
```

#### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate to next item |
| `k` / `↑` | Navigate to previous item |
| `a` | Approve selected item |
| `d` | Deny selected item |
| `Shift+A` | Approve all in current cluster |
| `Shift+D` | Deny all in current cluster |
| `v` | View details panel |
| `g` | Group similar requests |
| `f` | Open filter panel |
| `1-4` | Filter by priority level |
| `/` | Focus search |
| `Esc` | Clear selection / close panel |

#### Semantic Clustering

Requests are automatically grouped by:
- **Operation type** (read, write, delete, execute)
- **Target resource** (database, API, filesystem, network)
- **Agent cluster** (agents working on related tasks)
- **Risk profile** (similar risk assessments)

Bulk actions apply to entire clusters with single confirmation.

#### Priority Sorting Algorithm

```
Priority Score = (Risk Level × 3) + (Confidence Inverse × 2) + (Age in Minutes × 0.1)

Where:
- Risk Level: Critical=4, High=3, Medium=2, Low=1
- Confidence Inverse: (100 - Confidence%) / 25
- Age: Minutes since request
```

---

### 2.d Agent Sight (Live Feeds)

Real-time visualization of what agents are "seeing" and processing.

#### Grid Layout

```
┌────────────────────────────────────────────────────────────────────┐
│  AGENT SIGHT - Live Feeds                     [2x2] [3x3] [4x4]   │
├────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐  ┌─────────────────────┐                 │
│  │ agent-vision-01     │  │ agent-vision-02     │                 │
│  │ ┌─────────────────┐ │  │ ┌─────────────────┐ │                 │
│  │ │                 │ │  │ │                 │ │                 │
│  │ │  [Live Feed]    │ │  │ │  [Live Feed]    │ │                 │
│  │ │  + Saliency     │ │  │ │  + Saliency     │ │                 │
│  │ │    Overlay      │ │  │ │    Overlay      │ │                 │
│  │ │                 │ │  │ │                 │ │                 │
│  │ └─────────────────┘ │  │ └─────────────────┘ │                 │
│  │ ● REC  00:03:42     │  │ ● REC  00:01:15     │                 │
│  └─────────────────────┘  └─────────────────────┘                 │
│  ┌─────────────────────┐  ┌─────────────────────┐                 │
│  │ agent-vision-03     │  │ agent-vision-04     │                 │
│  │ ┌─────────────────┐ │  │ ┌─────────────────┐ │                 │
│  │ │                 │ │  │ │                 │ │                 │
│  │ │  [Live Feed]    │ │  │ │  [Live Feed]    │ │                 │
│  │ │                 │ │  │ │                 │ │                 │
│  │ └─────────────────┘ │  │ └─────────────────┘ │                 │
│  │ ○ PAUSED            │  │ ● REC  00:07:22     │                 │
│  └─────────────────────┘  └─────────────────────┘                 │
└────────────────────────────────────────────────────────────────────┘
```

#### Saliency Overlay

Visual heatmap showing where the agent's attention is focused:

- **Red/Yellow hotspots** indicate high attention areas
- **Cool blue areas** indicate peripheral awareness
- **Opacity slider** (0-100%) to adjust overlay visibility
- **Threshold control** to show only above-threshold attention

#### Full-Screen Mode

- **Double-click** any feed to expand
- **Picture-in-picture** keeps thumbnail of other feeds
- **Side panel** shows agent metadata and recent actions
- **Escape** or click outside to return to grid

#### Recording & Playback Controls

```
┌────────────────────────────────────────────────────────────────┐
│  ◀◀  │  ◀  │  ⏸  │  ▶  │  ▶▶  │  ●REC  │  ○──────●────○  │
│  -10s  -1s   Pause  +1s  +10s    Toggle    Timeline scrubber   │
└────────────────────────────────────────────────────────────────┘

Storage: Last 24 hours retained │ Export: MP4, GIF, PNG sequence
```

---

### 2.e Metrics Dashboard

Comprehensive KPI monitoring with real-time updates and historical trends.

#### KPI Cards Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│  METRICS OVERVIEW                                    Last updated: 2s│
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐│
│  │ COST/TASK    │ │ P50 LATENCY  │ │ P95 LATENCY  │ │ P99 LATENCY  ││
│  │              │ │              │ │              │ │              ││
│  │   $0.0234    │ │    124ms     │ │    342ms     │ │    891ms     ││
│  │   ▼ 12%      │ │   ▼ 8%       │ │   ▲ 3%       │ │   ▲ 15%      ││
│  │  ╱╲_/╲_╱╲    │ │  _/╲_╱╲_/    │ │  ╱╲_╱╲__╱    │ │  __/╲_╱╲_    ││
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘│
│                                                                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐│
│  │ SUCCESS RATE │ │ HALLUC. RATE │ │ ACTIVE AGENTS│ │ QUEUE DEPTH  ││
│  │              │ │              │ │              │ │              ││
│  │   96.7%      │ │    2.1%      │ │     847      │ │     156      ││
│  │   ▲ 2.1%     │ │   ▼ 0.8%     │ │   ▲ 23       │ │   ▼ 12       ││
│  │  ╱╲_/╲__/╲   │ │  ╲_╱╲_/╲_    │ │  _/╲_/╲_╱    │ │  ╲__╱╲_╱╲    ││
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘│
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### Sparkline Specifications

- **Width**: 80px embedded in card
- **Height**: 24px
- **Time ranges**: 24h (default), 7d, 30d (toggle buttons)
- **Data points**: 48 points for 24h (30-min intervals), 168 for 7d, 30 for 30d
- **Hover**: Shows exact value and timestamp

#### Trend Indicators

| Symbol | Meaning | Color |
|--------|---------|-------|
| ▲ | Increasing | Green (good) or Red (bad, context-dependent) |
| ▼ | Decreasing | Red (good) or Green (bad, context-dependent) |
| ● | Stable (< 1% change) | Gray |

#### Drill-Down Capability

Clicking any KPI card opens detailed view:

```
┌─────────────────────────────────────────────────────────────────────┐
│  SUCCESS RATE - Detailed View                              [✕ Close]│
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Current: 96.7%    Target: 95.0%    Status: ✓ Above Target          │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                         TREND CHART                            │ │
│  │  100%│      ╱╲    ╱╲                                          │ │
│  │   95%│ ╱╲_╱╲  ╲__╱  ╲_╱╲__╱╲_╱╲____/╲                        │ │
│  │   90%│                                                        │ │
│  │   85%│________________________________________________        │ │
│  │       00:00  04:00  08:00  12:00  16:00  20:00  Now           │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  Breakdown by Agent Type:                                            │
│  ├─ Orchestrators: 98.2%  ████████████████████░░                    │
│  ├─ Workers:       95.8%  ███████████████████░░░                    │
│  ├─ Validators:    97.1%  ███████████████████░░░                    │
│  └─ Specialists:   94.3%  ██████████████████░░░░                    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

### 2.f Intervention Panel

Emergency and routine controls for human oversight of agent behavior.

#### Panel Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│  INTERVENTION CONTROLS                           Target: [Select ▼] │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │  💬 NUDGE                                                       ││
│  │  Send system message to influence agent behavior                ││
│  │  ┌─────────────────────────────────────────────────────────────┐││
│  │  │ Enter message...                                            │││
│  │  └─────────────────────────────────────────────────────────────┘││
│  │  [Send to Selected] [Send to All in Cluster]                    ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │  ⏸ PAUSE & PATCH                                                ││
│  │  Freeze agent, modify state, resume execution                   ││
│  │  ┌─────────────────────┐ ┌─────────────────────────────────────┐││
│  │  │ [Pause Selected]    │ │ State Editor (JSON)                 │││
│  │  │ [View State]        │ │ {                                   │││
│  │  │ [Edit State]        │ │   "memory": [...],                  │││
│  │  │ [Resume]            │ │   "context": {...}                  │││
│  │  └─────────────────────┘ │ }                                   │││
│  │                          └─────────────────────────────────────┘││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │  🎮 TAKEOVER                                                    ││
│  │  Human teleoperation - you control the agent directly           ││
│  │  [Initiate Takeover]  Status: Available                         ││
│  │  Warning: Agent will be paused during takeover                  ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │  🛑 KILL SWITCH                                    [ARMED: OFF] ││
│  │  Emergency halt - immediately terminates agent(s)               ││
│  │  ┌─────────────────────────────────────────────────────────────┐││
│  │  │         [KILL SELECTED]    [KILL ALL AGENTS]               │││
│  │  │              ⚠️ Requires confirmation                        │││
│  │  └─────────────────────────────────────────────────────────────┘││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### Confirmation Dialogs

Critical actions require explicit confirmation:

```
┌─────────────────────────────────────────────────────────────────────┐
│  ⚠️  CONFIRM KILL SWITCH                                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  You are about to terminate 847 active agents.                      │
│                                                                      │
│  This action will:                                                   │
│  • Immediately halt all agent processes                              │
│  • Cancel 156 pending tasks                                          │
│  • Trigger rollback procedures for 23 in-progress writes            │
│                                                                      │
│  Type "CONFIRM KILL" to proceed:                                    │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │                                                                  ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│                              [Cancel]  [Execute Kill Switch]        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### Keyboard Shortcuts for Interventions

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+N` | Open nudge panel |
| `Ctrl+Shift+P` | Pause selected agents |
| `Ctrl+Shift+R` | Resume paused agents |
| `Ctrl+Shift+T` | Initiate takeover |
| `Ctrl+Shift+K` | Open kill switch (requires additional confirmation) |

---

## 3. Technology Stack

### 3.1 Core Framework

| Technology | Version | Purpose |
|------------|---------|---------|
| **React** | 18.3+ | UI framework with concurrent features |
| **TypeScript** | 5.4+ | Type safety (strict mode enabled) |
| **Vite** | 5.x | Build tool with fast HMR (<50ms) |

#### TypeScript Configuration

```json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "exactOptionalPropertyTypes": true,
    "noUncheckedIndexedAccess": true
  }
}
```

### 3.2 State Management

| Library | Purpose |
|---------|---------|
| **Zustand** | Global client state (UI state, user preferences) |
| **TanStack Query** | Server state (caching, background refetch, optimistic updates) |

#### Zustand Store Structure

```typescript
interface PanopticonStore {
  // UI State
  selectedAgents: Set<string>;
  gridZoom: number;
  gridPan: { x: number; y: number };
  activePanel: 'grid' | 'timeline' | 'approvals' | 'metrics';

  // User Preferences
  theme: 'dark' | 'light' | 'system';
  saliencyOverlayOpacity: number;
  confidenceThreshold: number;

  // Actions
  selectAgent: (id: string) => void;
  clearSelection: () => void;
  setZoom: (zoom: number) => void;
  setPan: (x: number, y: number) => void;
}
```

### 3.3 Real-Time Communication

| Technology | Purpose |
|------------|---------|
| **WebSocket** | Bi-directional real-time updates |
| **Auto-reconnect** | Exponential backoff (1s, 2s, 4s, 8s, max 30s) |
| **Message queue** | Offline message buffering |

#### WebSocket Message Types

```typescript
type WSMessage =
  | { type: 'agent:update'; payload: AgentState }
  | { type: 'agent:batch'; payload: AgentState[] }
  | { type: 'metric:update'; payload: MetricSnapshot }
  | { type: 'approval:new'; payload: ApprovalRequest }
  | { type: 'approval:resolved'; payload: { id: string; status: 'approved' | 'denied' } }
  | { type: 'alert:new'; payload: Alert }
  | { type: 'heartbeat'; payload: { serverTime: number } };
```

### 3.4 Visualization Libraries

| Library | Use Case | Rendering |
|---------|----------|-----------|
| **D3.js** | Hex grid (<500 agents: SVG, 500+: Canvas) | SVG/Canvas |
| **Plotly.js** | Interactive charts, drill-downs | SVG/WebGL |
| **Three.js** | Optional 3D agent visualization | WebGL |

#### Rendering Strategy Decision Tree

```
Agent Count Decision:
├── < 500 agents
│   └── Use SVG (better interaction, accessibility)
├── 500 - 5000 agents
│   └── Use Canvas 2D (good performance, acceptable quality)
└── > 5000 agents
    └── Use WebGL via Three.js (maximum performance)
```

### 3.5 UI Components

| Library | Purpose |
|---------|---------|
| **shadcn/ui** | Pre-built accessible components |
| **Radix Primitives** | Unstyled accessible primitives |
| **Tailwind CSS** | Utility-first styling |
| **Framer Motion** | Smooth animations |

### 3.6 Development Tools

```json
{
  "devDependencies": {
    "eslint": "^9.0.0",
    "prettier": "^3.2.0",
    "vitest": "^1.6.0",
    "@testing-library/react": "^15.0.0",
    "playwright": "^1.44.0",
    "storybook": "^8.1.0"
  }
}
```

---

## 4. Design System

### 4.1 Color Palette

#### Dark Mode (Primary)

```css
:root {
  /* Backgrounds */
  --bg-base: #0a0a0f;        /* Deepest background */
  --bg-surface: #12121a;      /* Card backgrounds */
  --bg-elevated: #1a1a2e;     /* Elevated elements, hovers */
  --bg-overlay: #252538;      /* Modal overlays */

  /* Primary (Blue) */
  --primary-50: #eff6ff;
  --primary-100: #dbeafe;
  --primary-200: #bfdbfe;
  --primary-300: #93c5fd;
  --primary-400: #60a5fa;
  --primary-500: #3b82f6;     /* Primary action color */
  --primary-600: #2563eb;
  --primary-700: #1d4ed8;
  --primary-800: #1e40af;
  --primary-900: #1e3a8a;

  /* Success (Green) */
  --success-50: #ecfdf5;
  --success-500: #10b981;     /* Success indicators */
  --success-600: #059669;
  --success-700: #047857;

  /* Warning (Amber) */
  --warning-50: #fffbeb;
  --warning-500: #f59e0b;     /* Warning indicators */
  --warning-600: #d97706;
  --warning-700: #b45309;

  /* Error (Red) */
  --error-50: #fef2f2;
  --error-500: #ef4444;       /* Error indicators */
  --error-600: #dc2626;
  --error-700: #b91c1c;

  /* Text */
  --text-primary: #f8fafc;    /* Primary text */
  --text-secondary: #94a3b8;  /* Secondary text */
  --text-tertiary: #64748b;   /* Disabled, hints */
  --text-inverse: #0f172a;    /* Text on light backgrounds */

  /* Borders */
  --border-subtle: #1e293b;
  --border-default: #334155;
  --border-strong: #475569;
}
```

#### Light Mode (Optional)

```css
:root.light {
  --bg-base: #ffffff;
  --bg-surface: #f8fafc;
  --bg-elevated: #f1f5f9;
  --text-primary: #0f172a;
  --text-secondary: #475569;
  /* ... etc */
}
```

### 4.2 Typography

#### Font Stack

```css
:root {
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
}
```

#### Type Scale

| Name | Size | Weight | Line Height | Use Case |
|------|------|--------|-------------|----------|
| `display` | 48px | 700 | 1.1 | Hero numbers |
| `h1` | 32px | 600 | 1.2 | Page titles |
| `h2` | 24px | 600 | 1.3 | Section headers |
| `h3` | 20px | 600 | 1.4 | Card titles |
| `h4` | 16px | 600 | 1.4 | Subsection headers |
| `body` | 14px | 400 | 1.5 | Default text |
| `body-sm` | 13px | 400 | 1.5 | Secondary text |
| `caption` | 12px | 400 | 1.4 | Labels, hints |
| `code` | 13px | 400 | 1.5 | Code snippets |

### 4.3 Spacing Scale

Base unit: **4px**

```css
:root {
  --space-0: 0px;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;
  --space-20: 80px;
  --space-24: 96px;
}
```

### 4.4 Border Radius

```css
:root {
  --radius-none: 0px;
  --radius-sm: 4px;
  --radius-default: 6px;     /* Buttons, inputs */
  --radius-md: 8px;
  --radius-lg: 12px;         /* Cards, modals */
  --radius-xl: 16px;
  --radius-full: 9999px;     /* Pills, avatars */
}
```

### 4.5 Shadows & Glows

```css
:root {
  /* Standard shadows */
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.3);
  --shadow-md: 0 4px 6px rgba(0, 0, 0, 0.4);
  --shadow-lg: 0 10px 15px rgba(0, 0, 0, 0.5);
  --shadow-xl: 0 20px 25px rgba(0, 0, 0, 0.6);

  /* Focus glows */
  --glow-primary: 0 0 0 3px rgba(59, 130, 246, 0.4);
  --glow-success: 0 0 0 3px rgba(16, 185, 129, 0.4);
  --glow-warning: 0 0 0 3px rgba(245, 158, 11, 0.4);
  --glow-error: 0 0 0 3px rgba(239, 68, 68, 0.4);

  /* Ambient glows (for cards, highlights) */
  --ambient-primary: 0 0 20px rgba(59, 130, 246, 0.15);
  --ambient-success: 0 0 20px rgba(16, 185, 129, 0.15);
}
```

### 4.6 Animation Tokens

```css
:root {
  /* Durations */
  --duration-instant: 50ms;
  --duration-fast: 100ms;
  --duration-normal: 200ms;
  --duration-slow: 300ms;
  --duration-slower: 500ms;

  /* Easings */
  --ease-default: cubic-bezier(0.4, 0, 0.2, 1);
  --ease-in: cubic-bezier(0.4, 0, 1, 1);
  --ease-out: cubic-bezier(0, 0, 0.2, 1);
  --ease-bounce: cubic-bezier(0.34, 1.56, 0.64, 1);
}
```

---

## 5. Component Architecture

### 5.1 Directory Structure

```
src/
├── app/
│   ├── layout.tsx                 # Root layout
│   ├── page.tsx                   # Main dashboard
│   └── (routes)/
│       ├── agents/
│       │   └── [id]/page.tsx      # Agent detail view
│       ├── timeline/page.tsx      # Timeline view
│       ├── approvals/page.tsx     # Approval queue
│       └── settings/page.tsx      # User settings
│
├── components/
│   ├── agents/
│   │   ├── AgentGrid.tsx          # Hex grid visualization
│   │   ├── AgentCard.tsx          # Individual agent card
│   │   ├── AgentHoverCard.tsx     # Hover tooltip content
│   │   ├── AgentSight.tsx         # Live feed viewer
│   │   ├── AgentSightGrid.tsx     # Multi-feed grid layout
│   │   ├── SaliencyOverlay.tsx    # Attention heatmap
│   │   └── Minimap.tsx            # Grid navigation minimap
│   │
│   ├── timeline/
│   │   ├── GanttChart.tsx         # Main timeline component
│   │   ├── CriticalPath.tsx       # Critical path highlight
│   │   ├── TaskBar.tsx            # Individual task bar
│   │   ├── DependencyLine.tsx     # Connection between tasks
│   │   ├── TimeScrubber.tsx       # Playback controls
│   │   └── TimelineHeader.tsx     # Date/time navigation
│   │
│   ├── metrics/
│   │   ├── KPIPanel.tsx           # KPI cards container
│   │   ├── KPICard.tsx            # Individual KPI card
│   │   ├── Sparkline.tsx          # Inline trend chart
│   │   ├── TrendChart.tsx         # Detailed trend view
│   │   └── MetricDrilldown.tsx    # Expanded metric detail
│   │
│   ├── approvals/
│   │   ├── ApprovalQueue.tsx      # Main queue component
│   │   ├── ApprovalItem.tsx       # Individual request
│   │   ├── ApprovalCluster.tsx    # Grouped similar requests
│   │   ├── ApprovalFilters.tsx    # Filter controls
│   │   └── BulkActions.tsx        # Batch action toolbar
│   │
│   ├── interventions/
│   │   ├── InterventionPanel.tsx  # Main control panel
│   │   ├── NudgeForm.tsx          # Message input form
│   │   ├── PauseControls.tsx      # Pause/resume buttons
│   │   ├── StateEditor.tsx        # JSON state editor
│   │   ├── TakeoverDialog.tsx     # Takeover confirmation
│   │   └── KillSwitch.tsx         # Emergency halt button
│   │
│   ├── layout/
│   │   ├── Header.tsx             # Top navigation bar
│   │   ├── Sidebar.tsx            # Left navigation
│   │   ├── CommandPalette.tsx     # Cmd+K quick actions
│   │   └── NotificationCenter.tsx # Alert popover
│   │
│   └── ui/                        # shadcn/ui components
│       ├── button.tsx
│       ├── card.tsx
│       ├── dialog.tsx
│       ├── dropdown-menu.tsx
│       ├── input.tsx
│       ├── popover.tsx
│       ├── select.tsx
│       ├── slider.tsx
│       ├── tabs.tsx
│       ├── toast.tsx
│       └── tooltip.tsx
│
├── hooks/
│   ├── useAgentStream.ts          # Agent data subscription
│   ├── useWebSocket.ts            # WebSocket connection
│   ├── useKeyboardShortcuts.ts    # Global keyboard bindings
│   ├── useGridNavigation.ts       # Hex grid pan/zoom
│   ├── useApprovalQueue.ts        # Approval queue state
│   ├── useMetrics.ts              # Metric data fetching
│   ├── useLocalStorage.ts         # Persisted preferences
│   └── useMediaQuery.ts           # Responsive breakpoints
│
├── lib/
│   ├── api.ts                     # REST API client
│   ├── websocket.ts               # WebSocket manager
│   ├── hexGrid.ts                 # Hex grid math utilities
│   ├── criticalPath.ts            # CPM algorithm
│   ├── clustering.ts              # Semantic clustering
│   └── formatters.ts              # Number/date formatters
│
├── stores/
│   ├── agentStore.ts              # Agent state
│   ├── uiStore.ts                 # UI preferences
│   └── notificationStore.ts       # Alert queue
│
├── types/
│   ├── agent.ts                   # Agent-related types
│   ├── timeline.ts                # Task/dependency types
│   ├── metrics.ts                 # Metric data types
│   ├── approval.ts                # Approval request types
│   └── websocket.ts               # WS message types
│
└── styles/
    ├── globals.css                # Global styles, tokens
    └── animations.css             # Keyframe definitions
```

### 5.2 Component Specifications

#### AgentGrid.tsx

```typescript
interface AgentGridProps {
  agents: Agent[];
  selectedIds: Set<string>;
  onSelect: (id: string, multi: boolean) => void;
  onDoubleClick: (id: string) => void;
  zoom: number;
  pan: { x: number; y: number };
  onZoomChange: (zoom: number) => void;
  onPanChange: (pan: { x: number; y: number }) => void;
  showConfidenceOverlay: boolean;
  confidenceThreshold: number;
}

// Rendering mode automatically switches based on agent count
// < 500: SVG for better accessibility and interaction
// >= 500: Canvas for performance
```

#### ApprovalQueue.tsx

```typescript
interface ApprovalQueueProps {
  requests: ApprovalRequest[];
  onApprove: (ids: string[]) => void;
  onDeny: (ids: string[]) => void;
  onViewDetails: (id: string) => void;
  clusters: ApprovalCluster[];
  filters: ApprovalFilters;
  onFiltersChange: (filters: ApprovalFilters) => void;
}
```

---

## 6. Wireframes

### 6.1 Main Dashboard Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PANOPTICON PROTOCOL                    🔍 Search   🔔 12   👤 Operator     │
├─────────────────────────────────────────────────────────────────────────────┤
│ ┌───┐                                                                       │
│ │ ≡ │  ┌─────────────────────────────────────────────────────────────────┐ │
│ ├───┤  │                                                                 │ │
│ │ ⬡ │  │                     AGENT HEX GRID                              │ │
│ │   │  │                                                                 │ │
│ │ 📊│  │    ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡               │ │
│ │   │  │   ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡              │ │
│ │ 📅│  │    ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡       ┌────┐  │ │
│ │   │  │   ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡     │    │  │ │
│ │ ✓ │  │    ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡       │mini│  │ │
│ │   │  │   ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡     │map │  │ │
│ │ 👁 │  │                                                       └────┘  │ │
│ │   │  │  Zoom: [- ═══●══ +]   Selected: 3 agents                       │ │
│ │ ⚙ │  └─────────────────────────────────────────────────────────────────┘ │
│ └───┘                                                                       │
│        ┌──────────────────────────────────────────┐ ┌──────────────────────┐│
│        │  METRICS OVERVIEW                        │ │  APPROVAL QUEUE      ││
│        │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐        │ │  ┌──────────────────┐││
│        │  │$0.02│ │124ms│ │96.7%│ │ 847 │        │ │  │ ⚠ DROP TABLE     │││
│        │  │Cost │ │ P50 │ │Succ.│ │Activ│        │ │  │ ● API Write      │││
│        │  └─────┘ └─────┘ └─────┘ └─────┘        │ │  │ ● File Delete    │││
│        └──────────────────────────────────────────┘ │  └──────────────────┘││
│                                                     │  Pending: 23         ││
│                                                     └──────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘

Legend:
≡  Menu           ⬡  Agents        📊 Metrics       📅 Timeline
✓  Approvals      👁  Agent Sight   ⚙  Settings
```

### 6.2 Agent Grid Detail View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ← Back to Grid    AGENT: orchestrator-7f3a                    [⋮ Actions] │
├─────────────────────────────────────────────────────────────────────────────┤
│ ┌───────────────────────────────────┐ ┌───────────────────────────────────┐ │
│ │  LIVE VIEW                        │ │  AGENT DETAILS                    │ │
│ │  ┌─────────────────────────────┐  │ │                                   │ │
│ │  │                             │  │ │  ID: orchestrator-7f3a            │ │
│ │  │                             │  │ │  Type: Orchestrator               │ │
│ │  │    [Screen Capture]         │  │ │  Status: ● BUSY                   │ │
│ │  │    + Saliency Overlay       │  │ │  Uptime: 2h 34m 12s               │ │
│ │  │                             │  │ │                                   │ │
│ │  │                             │  │ │  Current Task:                    │ │
│ │  └─────────────────────────────┘  │ │  "Analyzing Q4 customer feedback" │ │
│ │  ● REC 00:45:23  [⏸] [⏹]         │ │                                   │ │
│ └───────────────────────────────────┘ │  Progress: ████████░░ 78%         │ │
│                                       │                                   │ │
│ ┌───────────────────────────────────┐ │  Confidence: 87%                  │ │
│ │  METRICS                          │ │  Tokens: 12,847 / 32,000          │ │
│ │                                   │ │  Est. Cost: $0.0342               │ │
│ │  Success Rate    Avg Latency      │ │                                   │ │
│ │  ███████░░ 94%   ██████░░ 234ms   │ │  Success Rate (24h): 94.2%        │ │
│ │                                   │ │  Tasks Completed: 127             │ │
│ │  Token Usage     Cost Today       │ │  Avg Task Duration: 4m 23s        │ │
│ │  ██████░░░ 67%   $1.23            │ │                                   │ │
│ └───────────────────────────────────┘ └───────────────────────────────────┘ │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │  RECENT ACTIVITY                                                        │ │
│ │  ├─ 00:02:34  Completed subtask: "Extract sentiment scores"            │ │
│ │  ├─ 00:05:12  Called API: sentiment-analysis-v2                        │ │
│ │  ├─ 00:08:45  Spawned child agent: worker-sentiment-8b2c               │ │
│ │  └─ 00:12:01  Received data: 2,847 feedback entries                    │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │  INTERVENTIONS        [Nudge]  [Pause]  [Takeover]  [🛑 Kill]          │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Approval Queue View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  APPROVAL QUEUE                                           Pending: 23      │
├─────────────────────────────────────────────────────────────────────────────┤
│ ┌───────────────────────────┐ ┌───────────────────────────────────────────┐ │
│ │  FILTERS                  │ │  SEMANTIC CLUSTERS                        │ │
│ │                           │ │                                           │ │
│ │  Priority:                │ │  ┌─────────┐ ┌─────────┐ ┌─────────┐     │ │
│ │  [x] Critical (3)         │ │  │Database │ │  API    │ │  File   │     │ │
│ │  [x] High (7)             │ │  │Writes(8)│ │Calls(12)│ │ Ops(3)  │     │ │
│ │  [x] Medium (8)           │ │  └─────────┘ └─────────┘ └─────────┘     │ │
│ │  [x] Low (5)              │ │                                           │ │
│ │                           │ │  [Approve All Similar] [Deny All Similar] │ │
│ │  Agent Type:              │ └───────────────────────────────────────────┘ │
│ │  [x] Orchestrators        │                                               │
│ │  [x] Workers              │ ┌───────────────────────────────────────────┐ │
│ │  [x] Validators           │ │  ⚠ CRITICAL                               │ │
│ │                           │ │                                           │ │
│ │  Time Range:              │ │  ┌─────────────────────────────────────┐ │ │
│ │  [Last 24 hours    ▼]     │ │  │ ► agent-db-writer                   │ │ │
│ │                           │ │  │   wants to: DROP TABLE users        │ │ │
│ │  [Clear Filters]          │ │  │   Risk: CRITICAL | Confidence: 34%  │ │ │
│ └───────────────────────────┘ │  │   Requested: 2m ago                 │ │ │
│                               │  │   ┌───────────────────────────────┐ │ │ │
│ ┌───────────────────────────┐ │  │   │ Justification: "Cleaning up   │ │ │ │
│ │  KEYBOARD SHORTCUTS       │ │  │   │ deprecated user records per   │ │ │ │
│ │                           │ │  │   │ task #4521"                   │ │ │ │
│ │  j/k - Navigate           │ │  │   └───────────────────────────────┘ │ │ │
│ │  a   - Approve            │ │  │                                     │ │ │
│ │  d   - Deny               │ │  │   [A] Approve  [D] Deny  [V] View   │ │ │
│ │  v   - View details       │ │  └─────────────────────────────────────┘ │ │
│ │  g   - Group similar      │ │                                           │ │
│ │  /   - Search             │ │  ┌─────────────────────────────────────┐ │ │
│ │  1-4 - Filter priority    │ │  │   agent-api-caller                  │ │ │
│ │                           │ │  │   wants to: POST /payments/charge   │ │ │
│ │  Shift+A - Approve all    │ │  │   Risk: CRITICAL | Confidence: 89%  │ │ │
│ │  Shift+D - Deny all       │ │  │   Requested: 5m ago                 │ │ │
│ └───────────────────────────┘ │  └─────────────────────────────────────┘ │ │
│                               │                                           │ │
│                               │  ► HIGH (7)                               │ │
│                               │  ► MEDIUM (8)                             │ │
│                               │  ► LOW (5)                                │ │
│                               └───────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.4 Timeline View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CRITICAL PATH TIMELINE                    [Hour] [Day] [Week]  🔍 Search   │
├─────────────────────────────────────────────────────────────────────────────┤
│  ◀ 2026-01-29 ▶                                                             │
│                                                                             │
│      │ 00:00  │ 04:00  │ 08:00  │ 12:00  │ 16:00  │ 20:00  │ 00:00 │       │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│      │        │        │   ▼ NOW                                    │       │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│ Data │████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│       │
│Ingest│ Completed                  Scheduled                        │       │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│ ETL  │        │████████████████████████████▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░│       │
│ Proc │        │ Completed              In Progress    Remaining    │       │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│ ML   │        │        │        │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░│ ← Crit│
│Train │        │        │        │ ★ CRITICAL PATH ★               │  Path │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│Report│        │        │        │        │        │░░░░░░░░░░░░░░░│       │
│ Gen  │        │        │        │        │        │ Waiting        │       │
│ ─────┼────────┼────────┼────────┼────────┼────────┼────────┼───────┼────── │
│      │        │        │        │        │        │        │       │       │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ ◀◀ │ ◀ │ ⏸ │ ▶ │ ▶▶ │   ─────────────●──────────────────   │ 1x ▼│   │
│  │ -1h  -5m     +5m +1h           08:00              16:00              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Legend: ████ Complete  ▓▓▓▓ In Progress  ░░░░ Scheduled  ──── Dependency  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Accessibility (WCAG 2.1 AA)

### 7.1 Keyboard Navigation

#### Global Navigation

| Key | Action |
|-----|--------|
| `Tab` | Move focus to next interactive element |
| `Shift+Tab` | Move focus to previous element |
| `Enter` / `Space` | Activate focused element |
| `Escape` | Close modal/popover, clear selection |
| `Cmd/Ctrl+K` | Open command palette |
| `?` | Open keyboard shortcuts help |

#### Agent Grid Navigation

| Key | Action |
|-----|--------|
| `Arrow keys` | Move selection in grid |
| `Home` | Jump to first agent |
| `End` | Jump to last agent |
| `Page Up/Down` | Jump by 10 agents |
| `Enter` | Open selected agent details |
| `Space` | Toggle selection |
| `Ctrl+A` | Select all visible agents |

#### Approval Queue Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate items |
| `a` | Approve |
| `d` | Deny |
| `v` | View details |
| `[` / `]` | Collapse/expand groups |

### 7.2 Screen Reader Support

#### ARIA Implementation

```html
<!-- Agent Grid -->
<div role="grid" aria-label="Agent monitoring grid" aria-rowcount="1000">
  <div role="row" aria-rowindex="1">
    <div role="gridcell"
         aria-label="Agent orchestrator-7f3a, status: busy, confidence: 87%"
         aria-selected="false"
         tabindex="0">
      <!-- Hexagon visual -->
    </div>
  </div>
</div>

<!-- Live regions for updates -->
<div aria-live="polite" aria-atomic="true" class="sr-only">
  Agent orchestrator-7f3a status changed to error
</div>

<!-- Approval queue item -->
<article aria-labelledby="approval-123-title" role="listitem">
  <h3 id="approval-123-title">Critical: DROP TABLE request</h3>
  <p>Agent db-writer requests permission to drop users table</p>
  <div role="group" aria-label="Actions">
    <button aria-keyshortcuts="a">Approve</button>
    <button aria-keyshortcuts="d">Deny</button>
  </div>
</article>
```

#### Live Announcements

Real-time updates are announced via ARIA live regions:

- **Polite**: Non-critical updates (agent status changes, metric updates)
- **Assertive**: Critical alerts (errors, kill switch activation)
- **Off**: High-frequency updates (position changes during pan/zoom)

### 7.3 Color Contrast Ratios

All text meets WCAG 2.1 AA requirements:

| Element | Foreground | Background | Ratio | Requirement |
|---------|------------|------------|-------|-------------|
| Primary text | #f8fafc | #0a0a0f | 18.3:1 | 4.5:1 (pass) |
| Secondary text | #94a3b8 | #0a0a0f | 7.2:1 | 4.5:1 (pass) |
| Primary button | #ffffff | #3b82f6 | 4.7:1 | 4.5:1 (pass) |
| Error text | #fca5a5 | #0a0a0f | 10.1:1 | 4.5:1 (pass) |
| Large headings | #f8fafc | #12121a | 16.1:1 | 3:1 (pass) |

### 7.4 Color-Independent Indicators

Every color-coded element includes a secondary indicator:

| State | Color | Secondary Indicator |
|-------|-------|---------------------|
| Busy | Blue | Animated pulse ring |
| Idle | Gray | Static, no ring |
| Error | Red | Exclamation icon, rapid pulse |
| Waiting | Orange | Clock icon, slow pulse |
| Success | Green | Checkmark icon |
| Paused | Purple | Pause icon |

### 7.5 Focus Indicators

All focusable elements have visible focus states:

```css
/* Global focus style */
:focus-visible {
  outline: none;
  box-shadow: var(--glow-primary);
}

/* High contrast mode */
@media (prefers-contrast: high) {
  :focus-visible {
    outline: 3px solid #ffffff;
    outline-offset: 2px;
  }
}
```

### 7.6 Motion Preferences

```css
/* Respect user motion preferences */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 8. Performance Targets

### 8.1 Loading Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| **First Contentful Paint** | < 1.0s | Lighthouse |
| **Largest Contentful Paint** | < 2.0s | Lighthouse |
| **Time to Interactive** | < 2.5s | Lighthouse |
| **Cumulative Layout Shift** | < 0.1 | Lighthouse |
| **Initial Bundle Size** | < 500KB gzipped | Build analysis |

### 8.2 Runtime Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| **Agent grid render (1000 agents)** | < 50ms | Canvas mode |
| **Agent grid render (100 agents)** | < 16ms | SVG mode |
| **WebSocket update to render** | < 16ms | Single frame budget |
| **Hover card display** | < 50ms | Including data fetch |
| **Panel transition** | < 200ms | 60fps animation |
| **Search/filter response** | < 100ms | Client-side filtering |

### 8.3 Bundle Optimization Strategy

```javascript
// Code splitting strategy
const AgentGrid = lazy(() => import('./components/agents/AgentGrid'));
const GanttChart = lazy(() => import('./components/timeline/GanttChart'));
const MetricDrilldown = lazy(() => import('./components/metrics/MetricDrilldown'));

// Tree-shaking friendly imports
import { format, formatDistance } from 'date-fns';
import { scaleLinear, scaleTime } from 'd3-scale';
import { select } from 'd3-selection';

// Bundle analysis targets
// - Main bundle: < 150KB
// - Vendor bundle: < 200KB
// - Route chunks: < 50KB each
// - Visualization libs: loaded on demand
```

### 8.4 Rendering Optimization

#### Agent Grid Performance

```typescript
// Virtual rendering for large grids
const visibleAgents = useMemo(() => {
  const viewport = calculateViewport(zoom, pan, containerSize);
  return agents.filter(agent => isInViewport(agent.position, viewport));
}, [agents, zoom, pan, containerSize]);

// Batch updates for multiple agent changes
const batchedUpdates = useCallback((updates: AgentUpdate[]) => {
  requestAnimationFrame(() => {
    updates.forEach(update => applyUpdate(update));
    render();
  });
}, []);

// Canvas rendering for 500+ agents
const renderCanvas = useCallback((ctx: CanvasRenderingContext2D) => {
  ctx.clearRect(0, 0, width, height);

  // Use OffscreenCanvas for heavy computation
  const offscreen = new OffscreenCanvas(width, height);
  const offCtx = offscreen.getContext('2d')!;

  visibleAgents.forEach(agent => {
    drawHexagon(offCtx, agent);
  });

  ctx.drawImage(offscreen, 0, 0);
}, [visibleAgents, width, height]);
```

#### WebSocket Optimization

```typescript
// Message batching and throttling
class WebSocketManager {
  private messageBuffer: WSMessage[] = [];
  private flushInterval = 16; // ~60fps

  scheduleFlush() {
    requestAnimationFrame(() => {
      const messages = this.messageBuffer.splice(0);
      if (messages.length > 0) {
        this.processMessages(messages);
      }
    });
  }

  processMessages(messages: WSMessage[]) {
    // Group by type for efficient processing
    const grouped = groupBy(messages, 'type');

    // Apply updates in single React batch
    ReactDOM.unstable_batchedUpdates(() => {
      grouped['agent:update']?.forEach(handleAgentUpdate);
      grouped['metric:update']?.forEach(handleMetricUpdate);
    });
  }
}
```

### 8.5 Memory Management

| Resource | Limit | Strategy |
|----------|-------|----------|
| Agent history | 1000 entries per agent | LRU cache |
| WebSocket buffer | 100 messages | Ring buffer |
| Canvas layers | 3 (base, agents, overlay) | Reuse buffers |
| Recording frames | 24h rolling | IndexedDB + cleanup |

### 8.6 Network Optimization

```typescript
// Prefetching strategy
const prefetchAgentDetails = (agentId: string) => {
  queryClient.prefetchQuery({
    queryKey: ['agent', agentId, 'details'],
    queryFn: () => fetchAgentDetails(agentId),
    staleTime: 30_000, // 30 seconds
  });
};

// Optimistic updates for approvals
const approveRequest = useMutation({
  mutationFn: (id: string) => api.approveRequest(id),
  onMutate: async (id) => {
    await queryClient.cancelQueries(['approvals']);
    const previous = queryClient.getQueryData(['approvals']);
    queryClient.setQueryData(['approvals'], (old: ApprovalRequest[]) =>
      old.filter(r => r.id !== id)
    );
    return { previous };
  },
  onError: (err, id, context) => {
    queryClient.setQueryData(['approvals'], context?.previous);
  },
});
```

---

## Appendix A: Component API Reference

### Agent Types

```typescript
interface Agent {
  id: string;
  name: string;
  type: 'orchestrator' | 'worker' | 'validator' | 'specialist';
  status: 'busy' | 'idle' | 'error' | 'waiting' | 'paused';
  position: { q: number; r: number }; // Hex coordinates
  task: {
    id: string;
    description: string;
    progress: number; // 0-100
    startedAt: string;
    estimatedCompletion: string;
  } | null;
  metrics: {
    confidence: number;
    tokensUsed: number;
    tokenLimit: number;
    costToDate: number;
    successRate: number;
    uptime: number; // seconds
  };
}
```

### Approval Request Types

```typescript
interface ApprovalRequest {
  id: string;
  agentId: string;
  agentName: string;
  action: {
    type: 'database' | 'api' | 'filesystem' | 'network' | 'system';
    operation: string;
    target: string;
    payload?: unknown;
  };
  risk: 'critical' | 'high' | 'medium' | 'low';
  confidence: number;
  justification: string;
  requestedAt: string;
  expiresAt: string;
  clusterId?: string;
  similarCount?: number;
}
```

### Timeline Task Types

```typescript
interface TimelineTask {
  id: string;
  name: string;
  agentId: string;
  status: 'completed' | 'in-progress' | 'scheduled' | 'blocked';
  startTime: string;
  endTime: string;
  duration: number; // minutes
  progress: number;
  dependencies: string[]; // task IDs
  isCriticalPath: boolean;
  slack: number; // minutes of available slack
}
```

---

## Appendix B: Design Tokens Export

```json
{
  "colors": {
    "background": {
      "base": "#0a0a0f",
      "surface": "#12121a",
      "elevated": "#1a1a2e"
    },
    "primary": {
      "500": "#3b82f6",
      "600": "#2563eb"
    },
    "success": {
      "500": "#10b981"
    },
    "warning": {
      "500": "#f59e0b"
    },
    "error": {
      "500": "#ef4444"
    },
    "text": {
      "primary": "#f8fafc",
      "secondary": "#94a3b8"
    }
  },
  "spacing": {
    "1": "4px",
    "2": "8px",
    "3": "12px",
    "4": "16px",
    "6": "24px",
    "8": "32px"
  },
  "radii": {
    "default": "6px",
    "lg": "12px"
  },
  "fonts": {
    "sans": "Inter, -apple-system, sans-serif",
    "mono": "JetBrains Mono, monospace"
  }
}
```

---

*Document generated for Project Apex - Panopticon Protocol*
*Frontend Architecture v1.0.0*
