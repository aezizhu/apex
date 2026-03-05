/* ─── Apex Swarm Dashboard ─── */
(() => {
  "use strict";

  const ROLE_STYLES = {
    coordinator:       { color: '#5E6AD2', icon: '\u25C6', label: 'Coord' },
    researcher:        { color: '#5B9AF5', icon: '\u25CE', label: 'Research' },
    analyst:           { color: '#9B8AEE', icon: '\u25A0', label: 'Analyst' },
    fact_checker:      { color: '#E8B931', icon: '\u25C8', label: 'Verify' },
    writer:            { color: '#3ECF8E', icon: '\u270E', label: 'Writer' },
    template_selector: { color: '#F59E0B', icon: '\u2630', label: 'Template' },
    layout_designer:   { color: '#EC4899', icon: '\u2B1A', label: 'Layout' },
    budget_planner:    { color: '#14B8A6', icon: '\u2261', label: 'Budget' },
    chapter_writer:    { color: '#6366F1', icon: '\u270D', label: 'Chapter' }
  };

  const PHASES = ['planning', 'researching', 'analyzing', 'fact_checking', 'writing', 'complete'];
  const PHASE_LABELS = {
    planning: 'Planning', researching: 'Researching', analyzing: 'Analyzing',
    fact_checking: 'Verifying', writing: 'Writing', complete: 'Complete'
  };

  // Report pipeline sub-stages (shown during the "writing" phase)
  const REPORT_STAGES = [
    'template_selection', 'layout_design', 'budgeting',
    'chapter_writing', 'stitching', 'rendering'
  ];
  const REPORT_STAGE_LABELS = {
    template_selection: 'Template', layout_design: 'Layout', budgeting: 'Budget',
    chapter_writing: 'Chapters', stitching: 'Stitching', rendering: 'Rendering'
  };

  // NOTE: escapeHtml is called before any HTML transforms in renderMarkdown,
  // ensuring user-provided text is sanitized before being inserted into the DOM.
  function escapeHtml(str) {
    const d = document.createElement("div");
    d.textContent = str;
    return d.innerHTML;
  }

  // renderMarkdown escapes ALL input via escapeHtml first, then applies
  // safe formatting transforms on the already-escaped string.
  function renderMarkdown(text) {
    if (!text) return "";
    let h = escapeHtml(text);
    h = h.replace(/```(\w*)\n([\s\S]*?)```/g, (_, l, c) => `<pre><code class="language-${l}">${c.trim()}</code></pre>`);
    h = h.replace(/`([^`]+)`/g, "<code>$1</code>");
    // Tables: consecutive lines starting/ending with |
    h = h.replace(/(^\|.+\|[ \t]*$\n?)+/gm, function(block) {
      const rows = block.trim().split('\n');
      if (rows.length < 2) return block;
      if (!/^\|[\s\-:]+\|$/.test(rows[1])) return block;
      const pr = (r) => r.split('|').slice(1, -1).map(c => c.trim());
      const hds = pr(rows[0]);
      let t = '<table><thead><tr>' + hds.map(c => '<th>' + c + '</th>').join('') + '</tr></thead><tbody>';
      for (let i = 2; i < rows.length; i++) {
        const cells = pr(rows[i]);
        t += '<tr>' + cells.map(c => '<td>' + c + '</td>').join('') + '</tr>';
      }
      return t + '</tbody></table>';
    });
    // Blockquotes (> is escaped to &gt; by escapeHtml)
    h = h.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>');
    h = h.replace(/<\/blockquote>\n<blockquote>/g, '<br/>');
    // Horizontal rules
    h = h.replace(/^-{3,}$/gm, '<hr/>');
    h = h.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    h = h.replace(/\*(.+?)\*/g, "<em>$1</em>");
    h = h.replace(/^### (.+)$/gm, "<h4>$1</h4>");
    h = h.replace(/^## (.+)$/gm, "<h3>$1</h3>");
    h = h.replace(/^# (.+)$/gm, "<h2>$1</h2>");
    h = h.replace(/^\s*[-*] (.+)$/gm, "<li>$1</li>");
    h = h.replace(/(<li>.*<\/li>\n?)+/g, (m) => `<ul>${m}</ul>`);
    h = h.replace(/^\s*\d+\. (.+)$/gm, "<li>$1</li>");
    h = h.replace(/\n\n/g, "</p><p>");
    h = h.replace(/\n/g, "<br/>");
    if (!h.startsWith("<")) h = `<p>${h}</p>`;
    return h;
  }

  // ── WebSocket client ──
  class SwarmClient {
    constructor() { this._h = {}; this._ws = null; this._sid = null; this._rt = null; this._ra = 0; }
    on(e, cb) { (this._h[e] = this._h[e] || []).push(cb); }
    _fire(e, d) { (this._h[e] || []).forEach(cb => cb(d)); }

    connect(sid) { this._sid = sid; this._ra = 0; this._open(); }

    _open() {
      if (this._ws) this._ws.close();
      const p = location.protocol === 'https:' ? 'wss' : 'ws';
      this._ws = new WebSocket(`${p}://${location.host}/ws/swarm/${this._sid}`);
      this._ws.onopen = () => { this._ra = 0; this._fire('connected', {}); };
      this._ws.onmessage = (e) => {
        try { const m = JSON.parse(e.data); this._fire(m.type || m.event, m); } catch {}
      };
      this._ws.onclose = () => {
        if (this._ra < 5) {
          this._ra++;
          this._rt = setTimeout(() => this._open(), Math.min(1000 * Math.pow(2, this._ra), 10000));
          this._fire('reconnecting', { attempt: this._ra });
        } else { this._fire('disconnected', {}); }
      };
      this._ws.onerror = () => this._fire('error', { message: 'Connection error' });
    }

    disconnect() { clearTimeout(this._rt); this._ra = 999; if (this._ws) this._ws.close(); this._ws = null; }
  }

  // ── Phase indicator ──
  class PhaseIndicator {
    constructor() { this._el = null; this._subEl = null; this._progressEl = null; }

    render(container) {
      this._el = document.createElement('div');
      this._el.className = 'phase-steps';
      PHASES.forEach((phase, i) => {
        if (i > 0) {
          const line = document.createElement('div');
          line.className = 'phase-line';
          line.dataset.after = phase;
          this._el.appendChild(line);
        }
        const step = document.createElement('div');
        step.className = 'phase-step';
        step.dataset.phase = phase;

        const circle = document.createElement('div');
        circle.className = 'phase-circle';
        const num = document.createElement('span');
        num.className = 'phase-num';
        num.textContent = String(i + 1);
        circle.appendChild(num);

        const label = document.createElement('span');
        label.className = 'phase-label';
        label.textContent = PHASE_LABELS[phase];

        step.appendChild(circle);
        step.appendChild(label);
        this._el.appendChild(step);
      });
      container.appendChild(this._el);

      // Report sub-stage indicator (hidden until writing phase)
      this._subEl = document.createElement('div');
      this._subEl.className = 'report-substages';
      this._subEl.style.display = 'none';
      REPORT_STAGES.forEach((stage) => {
        const chip = document.createElement('span');
        chip.className = 'substage-chip';
        chip.dataset.stage = stage;
        chip.textContent = REPORT_STAGE_LABELS[stage];
        this._subEl.appendChild(chip);
      });
      container.appendChild(this._subEl);

      // Progress bar
      this._progressEl = document.createElement('div');
      this._progressEl.className = 'report-progress-bar';
      this._progressEl.style.display = 'none';
      const fill = document.createElement('div');
      fill.className = 'report-progress-fill';
      const label = document.createElement('span');
      label.className = 'report-progress-label';
      this._progressEl.appendChild(fill);
      this._progressEl.appendChild(label);
      container.appendChild(this._progressEl);
    }

    setPhase(phase) {
      if (!this._el) return;
      const idx = PHASES.indexOf(phase);
      this._el.querySelectorAll('.phase-step').forEach((s, i) => {
        s.classList.remove('active', 'completed');
        if (i < idx) s.classList.add('completed');
        else if (i === idx) s.classList.add('active');
      });
      this._el.querySelectorAll('.phase-line').forEach((l) => {
        l.classList.toggle('completed', PHASES.indexOf(l.dataset.after) <= idx);
      });
      // Show sub-stage indicators during writing phase
      if (this._subEl) {
        const isWriting = phase === 'writing' || REPORT_STAGES.includes(phase);
        this._subEl.style.display = isWriting ? '' : 'none';
      }
      // Hide progress bar on complete
      if (phase === 'complete' && this._progressEl) {
        this._progressEl.style.display = 'none';
      }
    }

    setReportStage(stage, status) {
      if (!this._subEl) return;
      this._subEl.style.display = '';
      this._subEl.querySelectorAll('.substage-chip').forEach((chip) => {
        const chipStage = chip.dataset.stage;
        const chipIdx = REPORT_STAGES.indexOf(chipStage);
        const stageIdx = REPORT_STAGES.indexOf(stage);
        chip.classList.remove('active', 'completed');
        if (chipIdx < stageIdx) chip.classList.add('completed');
        else if (chipIdx === stageIdx) {
          chip.classList.add(status === 'completed' ? 'completed' : 'active');
        }
      });
    }

    setReportProgress(percent, message) {
      if (!this._progressEl) return;
      this._progressEl.style.display = '';
      const fill = this._progressEl.querySelector('.report-progress-fill');
      const label = this._progressEl.querySelector('.report-progress-label');
      if (fill) fill.style.width = Math.min(100, percent) + '%';
      if (label) label.textContent = message || (percent + '%');
    }
  }

  // ── Stream Panel — real-time agent output viewer ──
  class StreamPanel {
    constructor() {
      this._streams = {};       // agent_id → { name, role, text: string }
      this._selectedId = null;
      this._activeId = null;
      this._nameEl = null;
      this._badgeEl = null;
      this._statusEl = null;
      this._contentEl = null;
      this._textEl = null;
      this._emptyEl = null;
    }

    render() {
      this._nameEl = document.getElementById('stream-agent-name');
      this._badgeEl = document.getElementById('stream-role-badge');
      this._statusEl = document.getElementById('stream-status');
      this._contentEl = document.getElementById('stream-content');
      this._textEl = document.getElementById('stream-text');
      this._emptyEl = document.getElementById('stream-empty');
    }

    addChunk(agentId, agentName, role, content) {
      // Ensure stream buffer exists
      if (!this._streams[agentId]) {
        this._streams[agentId] = { name: agentName, role: role, text: '' };
      }
      this._streams[agentId].text += content;

      // Auto-switch to actively streaming agent
      const switched = this._activeId !== agentId;
      if (switched) {
        this._activeId = agentId;
        this._selectedId = agentId;
        this._updateHeader(agentId);
        // Full render when switching agents
        this._renderStreamFull(agentId);
      } else if (this._selectedId === agentId) {
        // Fast path: just append new chunk text
        this._appendChunk(content);
      }
    }

    select(agentId) {
      if (!this._streams[agentId]) return;
      this._selectedId = agentId;
      this._updateHeader(agentId);
      this._renderStreamFull(agentId);
    }

    setAgentDone(agentId) {
      if (this._activeId === agentId) {
        this._activeId = null;
        if (this._selectedId === agentId) {
          if (this._textEl) this._textEl.classList.remove('streaming');
          if (this._statusEl) this._statusEl.textContent = 'done';
        }
      }
    }

    getSelectedId() { return this._selectedId; }

    reset() {
      this._streams = {};
      this._selectedId = null;
      this._activeId = null;
      if (this._textEl) { this._textEl.style.display = 'none'; this._textEl.textContent = ''; this._textEl.classList.remove('streaming'); }
      if (this._emptyEl) this._emptyEl.style.display = '';
      if (this._nameEl) { this._nameEl.textContent = 'Waiting for agent...'; this._nameEl.classList.remove('active'); }
      if (this._badgeEl) this._badgeEl.style.display = 'none';
      if (this._statusEl) this._statusEl.textContent = '';
    }

    _updateHeader(agentId) {
      const stream = this._streams[agentId];
      if (!stream) return;
      const s = ROLE_STYLES[stream.role] || { color: '#5E6AD2', label: '?' };

      if (this._nameEl) {
        this._nameEl.textContent = stream.name;
        this._nameEl.classList.add('active');
      }
      if (this._badgeEl) {
        this._badgeEl.textContent = s.label;
        this._badgeEl.style.display = '';
        this._badgeEl.style.background = s.color + '20';
        this._badgeEl.style.color = s.color;
      }
      if (this._statusEl) {
        this._statusEl.textContent = (agentId === this._activeId) ? 'streaming...' : 'done';
      }
    }

    // Full render — used when switching agents
    _renderStreamFull(agentId) {
      const stream = this._streams[agentId];
      if (!stream) return;

      if (this._emptyEl) this._emptyEl.style.display = 'none';
      if (this._textEl) {
        this._textEl.style.display = '';
        this._textEl.textContent = stream.text;
        this._textEl.classList.toggle('streaming', agentId === this._activeId);
      }
      this._scrollToBottom();
    }

    // Fast append — used for new chunks on the currently selected stream
    _appendChunk(content) {
      if (this._emptyEl && this._emptyEl.style.display !== 'none') {
        this._emptyEl.style.display = 'none';
      }
      if (this._textEl) {
        if (this._textEl.style.display === 'none') this._textEl.style.display = '';
        // Append directly — no re-join of all chunks
        this._textEl.textContent += content;
        if (!this._textEl.classList.contains('streaming')) {
          this._textEl.classList.add('streaming');
        }
      }
      this._scrollToBottom();
    }

    _scrollToBottom() {
      if (this._contentEl) {
        requestAnimationFrame(() => {
          this._contentEl.scrollTop = this._contentEl.scrollHeight;
        });
      }
    }
  }

  // ── Agent panel ──
  class AgentPanel {
    constructor() { this._c = null; this._agents = {}; this._count = null; this._onSelect = null; this._accum = {}; }

    render() {
      this._c = document.getElementById('agents-list');
      this._count = document.getElementById('agent-count');
    }

    onSelect(cb) { this._onSelect = cb; }

    addAgent(agent) {
      if (this._agents[agent.id]) return;
      const s = ROLE_STYLES[agent.role] || { color: '#5E6AD2', icon: '\u25CB', label: '?' };

      const card = document.createElement('div');
      card.className = 'agent-card';
      card.dataset.agentId = agent.id;
      card.style.borderLeftColor = s.color;
      card.style.setProperty('--agent-color', s.color + '80');

      card.addEventListener('click', () => {
        if (this._onSelect) this._onSelect(agent.id);
        this._setSelected(agent.id);
      });

      const hdr = document.createElement('div');
      hdr.className = 'agent-header';

      const icon = document.createElement('span');
      icon.className = 'agent-icon';
      icon.style.color = s.color;
      icon.textContent = s.icon;

      const name = document.createElement('span');
      name.className = 'agent-name';
      name.textContent = agent.name || agent.id;

      const badge = document.createElement('span');
      badge.className = 'agent-role-badge';
      badge.style.background = s.color + '15';
      badge.style.color = s.color;
      badge.style.borderColor = s.color + '30';
      badge.textContent = s.label;

      hdr.appendChild(icon);
      hdr.appendChild(name);
      hdr.appendChild(badge);

      const sr = document.createElement('div');
      sr.className = 'agent-status-row';
      const dot = document.createElement('span');
      dot.className = 'status-dot idle';
      const stxt = document.createElement('span');
      stxt.className = 'agent-status-text';
      stxt.textContent = 'Idle';
      sr.appendChild(dot);
      sr.appendChild(stxt);

      const prev = document.createElement('div');
      prev.className = 'agent-output-preview';

      card.appendChild(hdr);
      card.appendChild(sr);
      card.appendChild(prev);
      this._c.appendChild(card);
      this._agents[agent.id] = card;
      this._accum[agent.id] = '';

      const n = Object.keys(this._agents).length;
      if (this._count) this._count.textContent = n;
    }

    updateAgent(id, data) {
      const card = this._agents[id];
      if (!card) return;
      if (data.status) {
        card.querySelector('.status-dot').className = 'status-dot ' + data.status;
        const labels = { working: 'Working', idle: 'Idle', done: 'Done', error: 'Error' };
        card.querySelector('.agent-status-text').textContent = labels[data.status] || data.status;
        card.classList.toggle('working', data.status === 'working');
      }
      if (data.chunk) {
        // Accumulate output text for preview
        this._accum[id] = (this._accum[id] || '') + data.chunk;
        const full = this._accum[id];
        const p = card.querySelector('.agent-output-preview');
        p.textContent = full.length > 80 ? '\u2026' + full.slice(-80) : full;
      }
    }

    setSelected(id) { this._setSelected(id); }

    _setSelected(id) {
      Object.values(this._agents).forEach(c => c.classList.remove('selected'));
      const card = this._agents[id];
      if (card) card.classList.add('selected');
    }

    reset() {
      this._agents = {};
      this._accum = {};
      if (this._count) this._count.textContent = '0';
    }
  }

  // ── Activity feed ──
  class ActivityFeed {
    constructor() { this._c = null; }
    render(container) { this._c = container; }

    addEntry(ev) {
      const el = document.createElement('div');
      el.className = 'activity-entry' + (ev.isError ? ' error' : '');
      const s = ROLE_STYLES[ev.role] || { color: '#5E6AD2', icon: '\u25CB' };
      const t = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });

      const ic = document.createElement('span');
      ic.className = 'activity-icon';
      ic.style.color = s.color;
      ic.textContent = s.icon;

      const ag = document.createElement('span');
      ag.className = 'activity-agent';
      ag.style.color = s.color;
      ag.textContent = ev.agent || '';

      const tx = document.createElement('span');
      tx.className = 'activity-text';
      tx.textContent = ev.text || '';

      const tm = document.createElement('span');
      tm.className = 'activity-time';
      tm.textContent = t;

      el.appendChild(ic);
      el.appendChild(ag);
      el.appendChild(tx);
      el.appendChild(tm);
      this._c.appendChild(el);
      this._c.scrollTop = this._c.scrollHeight;
    }
  }

  // ── Output panel ──
  class OutputPanel {
    constructor() { this._el = null; this._rawContent = ''; this._isRich = false; }
    render(el) { this._el = el; }

    // Legacy markdown report display
    setOutput(md) {
      this._rawContent = md;
      this._isRich = false;
      // safe: renderMarkdown calls escapeHtml on ALL input first
      this._render(renderMarkdown(md), 'output-content-rendered');
    }

    // Rich HTML report from server-side DocumentIR renderer
    // safe: HTML is generated server-side with html.escape() on all user content
    setRichOutput(htmlStr) {
      this._rawContent = htmlStr;
      this._isRich = true;
      this._render(htmlStr, 'output-content-rich');
    }

    _render(content, className) {
      if (!this._el) return;
      const empty = this._el.querySelector('.output-empty');
      if (empty) empty.remove();
      this._el.textContent = '';

      const div = document.createElement('div');
      div.className = className;
      // Content is pre-sanitized: renderMarkdown escapes input,
      // and rich HTML is server-generated with html.escape()
      div.innerHTML = content;

      const toolbar = document.createElement('div');
      toolbar.className = 'report-toolbar';

      const expandBtn = document.createElement('button');
      expandBtn.className = 'report-btn report-expand-btn';
      expandBtn.textContent = 'Expand';
      expandBtn.addEventListener('click', () => {
        if (this._isRich) {
          // Open rich HTML in a new tab via blob URL
          const blob = new Blob([this._rawContent], { type: 'text/html' });
          const url = URL.createObjectURL(blob);
          window.open(url, '_blank');
          setTimeout(() => URL.revokeObjectURL(url), 60000);
        } else {
          const overlay = document.getElementById('report-overlay');
          if (!overlay) return;
          const body = overlay.querySelector('.report-overlay-content');
          if (body) { body.textContent = ''; body.appendChild(div.cloneNode(true)); }
          overlay.classList.add('active');
        }
      });

      const copyBtn = document.createElement('button');
      copyBtn.className = 'report-btn report-copy-btn';
      copyBtn.textContent = 'Copy Report';
      copyBtn.addEventListener('click', () => {
        navigator.clipboard.writeText(this._rawContent).then(() => {
          copyBtn.textContent = 'Copied!';
          setTimeout(() => { copyBtn.textContent = 'Copy Report'; }, 2000);
        });
      });

      toolbar.appendChild(expandBtn);
      toolbar.appendChild(copyBtn);
      this._el.appendChild(toolbar);
      this._el.appendChild(div);
    }
  }

  // ── State ──
  let client = null;
  let currentSessionId = null;
  let phaseIndicator = null;
  let agentPanel = null;
  let activityFeed = null;
  let outputPanel = null;
  let streamPanel = null;

  window.initSwarm = function () {
    phaseIndicator = new PhaseIndicator();
    phaseIndicator.render(document.getElementById('phase-indicator'));

    agentPanel = new AgentPanel();
    agentPanel.render();

    activityFeed = new ActivityFeed();
    activityFeed.render(document.getElementById('activity-feed'));

    outputPanel = new OutputPanel();
    outputPanel.render(document.getElementById('output-content'));

    streamPanel = new StreamPanel();
    streamPanel.render();

    // Wire agent card clicks to stream panel
    agentPanel.onSelect((agentId) => {
      streamPanel.select(agentId);
    });
  };

  // Expose phase control for history viewing
  window.setPhaseComplete = function () {
    if (phaseIndicator) phaseIndicator.setPhase('complete');
  };

  // Expose report rendering so app.js can use OutputPanel with toolbar
  window.renderReportToPanel = function (md) {
    if (outputPanel) outputPanel.setOutput(md);
  };

  // Expose rich report rendering for IR-based HTML reports
  window.renderRichReportToPanel = function (htmlStr) {
    if (outputPanel) outputPanel.setRichOutput(htmlStr);
  };

  window.startSwarm = async function (query) {
    // Reset UI
    document.getElementById('activity-feed').querySelectorAll('.activity-entry').forEach(e => e.remove());
    document.getElementById('agents-list').querySelectorAll('.agent-card').forEach(e => e.remove());
    const oc = document.getElementById('output-content');
    oc.textContent = '';
    const emptyDiv = document.createElement('div');
    emptyDiv.className = 'output-empty';
    emptyDiv.textContent = 'Report will appear here once the swarm completes.';
    oc.appendChild(emptyDiv);
    if (agentPanel) agentPanel.reset();
    if (phaseIndicator) phaseIndicator.setPhase(null);
    if (streamPanel) streamPanel.reset();

    activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Starting swarm: "' + query + '"' });

    try {
      const res = await fetch('/api/swarm/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query })
      });

      if (!res.ok) {
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Error: ' + (await res.text()), isError: true });
        return;
      }

      const data = await res.json();
      if (client) client.disconnect();
      client = new SwarmClient();

      // Track which agents we've already logged as "started streaming"
      const loggedStreaming = new Set();

      // Hydrate agents that spawned before WS connected
      client.on('snapshot', (m) => {
        if (m.phase) phaseIndicator.setPhase(m.phase);
        if (m.agents) {
          Object.values(m.agents).forEach((a) => {
            agentPanel.addAgent({ id: a.id, name: a.name || a.id, role: a.role });
            agentPanel.updateAgent(a.id, { status: a.status });
          });
        }
      });

      client.on('agent_spawned', (m) => {
        agentPanel.addAgent({ id: m.agent_id, name: m.agent_name || m.agent_id, role: m.role });
        activityFeed.addEntry({ role: m.role, agent: m.agent_name || m.agent_id, text: 'joined as ' + m.role });
      });

      client.on('agent_status', (m) => {
        agentPanel.updateAgent(m.agent_id, { status: m.status });
        activityFeed.addEntry({ role: m.role, agent: m.agent_name || m.agent_id, text: m.message || ('status: ' + m.status) });
        if (m.status === 'done' || m.status === 'error') {
          streamPanel.setAgentDone(m.agent_id);
        }
      });

      client.on('phase_change', (m) => {
        phaseIndicator.setPhase(m.phase);
        activityFeed.addEntry({ role: 'coordinator', agent: 'Swarm', text: 'Phase: ' + (PHASE_LABELS[m.phase] || m.phase) });
      });

      client.on('agent_output', (m) => {
        // Feed stream panel with streaming chunks
        streamPanel.addChunk(m.agent_id, m.agent_name || m.agent_id, m.role, m.content);
        // Update agent card with accumulated preview and working status
        agentPanel.updateAgent(m.agent_id, { chunk: m.content, status: 'working' });
        // Auto-select agent card only when a NEW agent starts streaming
        if (!loggedStreaming.has(m.agent_id)) {
          loggedStreaming.add(m.agent_id);
          agentPanel.setSelected(m.agent_id);
          activityFeed.addEntry({ role: m.role, agent: m.agent_name || m.agent_id, text: 'started streaming output' });
        }
      });

      client.on('report_stage', (m) => {
        if (phaseIndicator) phaseIndicator.setReportStage(m.stage, m.status);
        const label = REPORT_STAGE_LABELS[m.stage] || m.stage;
        activityFeed.addEntry({
          role: 'coordinator', agent: 'Report',
          text: label + ' ' + (m.status || '')
        });
      });

      client.on('report_progress', (m) => {
        if (phaseIndicator) phaseIndicator.setReportProgress(m.progress || 0, m.message || '');
      });

      client.on('chapter_status', (m) => {
        const label = m.chapter_title || ('Chapter ' + (m.chapter_index + 1));
        activityFeed.addEntry({
          role: 'chapter_writer', agent: 'Chapter-Writer',
          text: label + ': ' + (m.status || '')
        });
      });

      client.on('swarm_complete', (m) => {
        phaseIndicator.setPhase('complete');
        // Use rich HTML renderer output if available, else fall back to markdown
        if (m.report_html) {
          outputPanel.setRichOutput(m.report_html);
        } else {
          outputPanel.setOutput(m.report || m.content || '');
        }
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Swarm complete' });
        const outputEl = document.getElementById('output-content');
        if (outputEl) outputEl.scrollIntoView({ behavior: 'smooth', block: 'start' });
        currentSessionId = null;
        if (window.onSwarmDone) window.onSwarmDone();
      });

      client.on('error', (m) => {
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Error: ' + (m.message || 'Unknown'), isError: true });
      });

      client.on('reconnecting', (m) => {
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Reconnecting (' + m.attempt + ')...' });
      });

      client.on('swarm_cancelled', (m) => {
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: m.message || 'Swarm stopped' });
        currentSessionId = null;
        if (window.onSwarmDone) window.onSwarmDone();
      });

      client.on('disconnected', () => {
        activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Disconnected', isError: true });
        currentSessionId = null;
      });

      currentSessionId = data.session_id;
      client.connect(data.session_id);
    } catch (err) {
      activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Failed: ' + err.message, isError: true });
    }
  };

  window.stopSwarm = async function () {
    if (!currentSessionId) return;
    try {
      await fetch('/api/swarm/' + currentSessionId + '/stop', { method: 'POST' });
      activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Stop requested' });
    } catch (err) {
      activityFeed.addEntry({ role: 'coordinator', agent: 'System', text: 'Stop failed: ' + err.message, isError: true });
    }
    if (client) client.disconnect();
    currentSessionId = null;
  };

  // Close report overlay on Escape or close button
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const overlay = document.getElementById('report-overlay');
      if (overlay && overlay.classList.contains('active')) {
        overlay.classList.remove('active');
      }
    }
  });
  const overlayCloseBtn = document.getElementById('report-overlay-close');
  if (overlayCloseBtn) {
    overlayCloseBtn.addEventListener('click', () => {
      const overlay = document.getElementById('report-overlay');
      if (overlay) overlay.classList.remove('active');
    });
  }
})();
