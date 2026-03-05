/* ─── Apex Chat + Mode Switching ─── */
(() => {
  "use strict";

  const messagesEl = document.getElementById("messages");
  const chatContainer = document.getElementById("chat-container");
  const userInput = document.getElementById("user-input");
  const sendBtn = document.getElementById("send-btn");
  const welcomeEl = document.getElementById("welcome");

  const swarmView = document.getElementById("swarm-view");
  const chatView = document.getElementById("chat-view");
  const modeSwitch = document.getElementById("mode-switch");

  // Welcome + Dashboard elements
  const swarmWelcome = document.getElementById("swarm-welcome");
  const swarmDashboard = document.getElementById("swarm-dashboard");

  // Welcome input bar
  const swarmQuery = document.getElementById("swarm-query");
  const swarmSendBtn = document.getElementById("swarm-send-btn");
  const swarmStopBtn = document.getElementById("swarm-stop-btn");

  // Dashboard input bar
  const swarmQueryDash = document.getElementById("swarm-query-dash");
  const swarmSendBtnDash = document.getElementById("swarm-send-btn-dash");
  const swarmStopBtnDash = document.getElementById("swarm-stop-btn-dash");

  let conversationHistory = [];
  let isStreaming = false;
  let swarmInitialized = false;

  // ── Initialize swarm on load (swarm is default view) ──
  if (window.initSwarm) {
    window.initSwarm();
    swarmInitialized = true;
  }

  // ── Load history on startup ──
  loadHistory();

  // Reset stop/send buttons when swarm finishes
  window.onSwarmDone = function () {
    setSwarmRunning(false);
    loadHistory();  // refresh history list
  };

  // ── Mode switching via segmented control ──
  if (modeSwitch) {
    modeSwitch.addEventListener("click", (e) => {
      const btn = e.target.closest(".mode-btn");
      if (!btn) return;
      const mode = btn.dataset.mode;

      modeSwitch.querySelectorAll(".mode-btn").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");

      swarmView.classList.toggle("active", mode === "swarm");
      chatView.classList.toggle("active", mode === "chat");

      if (mode === "swarm" && !swarmInitialized) {
        if (window.initSwarm) window.initSwarm();
        swarmInitialized = true;
      }
      if (mode === "chat" && userInput) {
        userInput.focus();
      }
    });
  }

  // ── Welcome → Dashboard transition ──
  function showDashboard() {
    if (swarmWelcome) swarmWelcome.style.display = "none";
    if (swarmDashboard) swarmDashboard.style.display = "";
  }

  // ── Swarm running state ──
  function setSwarmRunning(running) {
    // Welcome bar
    if (swarmStopBtn) swarmStopBtn.style.display = running ? "" : "none";
    if (swarmSendBtn) swarmSendBtn.style.display = running ? "none" : "";
    // Dashboard bar
    if (swarmStopBtnDash) swarmStopBtnDash.style.display = running ? "" : "none";
    if (swarmSendBtnDash) swarmSendBtnDash.style.display = running ? "none" : "";
  }

  // ── Get active query value from whichever input is visible ──
  function getQueryValue() {
    if (swarmWelcome && swarmWelcome.style.display !== "none") {
      return (swarmQuery && swarmQuery.value.trim()) || "";
    }
    return (swarmQueryDash && swarmQueryDash.value.trim()) || "";
  }

  function clearQueryInput() {
    if (swarmQuery) { swarmQuery.value = ""; swarmQuery.style.height = "auto"; }
    if (swarmQueryDash) { swarmQueryDash.value = ""; swarmQueryDash.style.height = "auto"; }
    if (swarmSendBtn) swarmSendBtn.disabled = true;
    if (swarmSendBtnDash) swarmSendBtnDash.disabled = true;
  }

  // ── Swarm input handling — wire both input bars ──
  function wireInput(textarea, sendButton) {
    if (!textarea) return;
    textarea.addEventListener("input", () => {
      textarea.style.height = "auto";
      textarea.style.height = Math.min(textarea.scrollHeight, 120) + "px";
      if (sendButton) sendButton.disabled = textarea.value.trim() === "";
    });
    textarea.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        launchSwarmFrom(textarea);
      }
    });
    if (sendButton) {
      sendButton.addEventListener("click", () => launchSwarmFrom(textarea));
    }
  }

  wireInput(swarmQuery, swarmSendBtn);
  wireInput(swarmQueryDash, swarmSendBtnDash);

  // ── Wire chat input ──
  if (userInput) {
    userInput.addEventListener("input", () => {
      userInput.style.height = "auto";
      userInput.style.height = Math.min(userInput.scrollHeight, 120) + "px";
      if (sendBtn) sendBtn.disabled = userInput.value.trim() === "";
    });
    userInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
      }
    });
  }
  if (sendBtn) {
    sendBtn.addEventListener("click", () => sendMessage());
  }

  // Wire stop buttons
  function wireStop(stopBtn) {
    if (!stopBtn) return;
    stopBtn.addEventListener("click", () => {
      if (window.stopSwarm) window.stopSwarm();
      setSwarmRunning(false);
    });
  }
  wireStop(swarmStopBtn);
  wireStop(swarmStopBtnDash);

  function launchSwarmFrom(textarea) {
    const q = textarea ? textarea.value.trim() : "";
    if (!q) return;
    launchSwarmWithQuery(q);
  }

  function launchSwarmWithQuery(q) {
    if (!q) return;
    showDashboard();
    setSwarmRunning(true);
    if (window.startSwarm) window.startSwarm(q);
    clearQueryInput();
  }

  // ── History ──
  async function loadHistory() {
    const list = document.getElementById("history-list");
    const section = document.getElementById("swarm-history");
    if (!list || !section) return;
    try {
      const res = await fetch("/api/swarm/history");
      if (!res.ok) return;
      const reports = await res.json();
      if (!reports.length) {
        section.style.display = "none";
        return;
      }
      section.style.display = "";
      list.innerHTML = "";
      reports.forEach((r) => {
        const item = document.createElement("div");
        item.className = "history-item";
        item.dataset.reportId = r.id;

        const query = document.createElement("span");
        query.className = "history-query";
        query.textContent = r.query;

        const agents = document.createElement("span");
        agents.className = "history-agents";
        agents.textContent = r.agents_count + " agents";

        const meta = document.createElement("span");
        meta.className = "history-meta";
        const d = new Date(r.completed_at || r.created_at);
        meta.textContent = d.toLocaleDateString([], { month: "short", day: "numeric" }) + " " + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

        item.appendChild(query);
        item.appendChild(agents);
        item.appendChild(meta);
        list.appendChild(item);
      });
    } catch (err) {
      section.style.display = "none";
    }
  }

  async function viewReport(reportId) {
    try {
      const res = await fetch("/api/swarm/history/" + reportId);
      if (!res.ok) return;
      const data = await res.json();
      if (data.error) return;

      // Show dashboard with the saved report
      showDashboard();
      // Set phase to complete
      if (window.initSwarm && !swarmInitialized) {
        window.initSwarm();
        swarmInitialized = true;
      }
      // Display the report in the output panel — prefer rich IR HTML if available
      const oc = document.getElementById("output-content");
      if (data.report_html && window.renderRichReportToPanel) {
        window.renderRichReportToPanel(data.report_html);
      } else if (oc && window.renderReportToPanel) {
        window.renderReportToPanel(data.report || "");
      }
      // Open report for better readability
      if (data.report_html) {
        // Rich reports open in new tab via blob URL
        const blob = new Blob([data.report_html], { type: 'text/html' });
        const url = URL.createObjectURL(blob);
        window.open(url, '_blank');
        setTimeout(() => URL.revokeObjectURL(url), 60000);
      } else {
        const overlay = document.getElementById('report-overlay');
        if (overlay) {
          const body = overlay.querySelector('.report-overlay-content');
          if (body && window.renderReportToPanel) {
            body.textContent = '';
            const rendered = document.createElement('div');
            rendered.className = 'output-content-rendered';
            rendered.textContent = data.report || '';
            body.appendChild(rendered);
          }
          overlay.classList.add('active');
        }
      }
      // Update phase indicator to complete
      if (window.setPhaseComplete) window.setPhaseComplete();
      // Show query in activity
      const feed = document.getElementById("activity-feed");
      if (feed) {
        feed.innerHTML = "";
        const entry = document.createElement("div");
        entry.className = "activity-entry";
        entry.innerHTML = '<span class="activity-icon" style="color:#5E6AD2">\u25C6</span>' +
          '<span class="activity-agent" style="color:#5E6AD2">History</span>' +
          '<span class="activity-text">Viewing saved report: "' + escapeHtml(data.query) + '"</span>';
        feed.appendChild(entry);
      }
      // Show agent count
      const countEl = document.getElementById("agent-count");
      if (countEl) countEl.textContent = data.agents_count || "0";
      // Show report length in stream
      const streamName = document.getElementById("stream-agent-name");
      if (streamName) { streamName.textContent = "Saved Report"; streamName.classList.add("active"); }
      const streamText = document.getElementById("stream-text");
      const streamEmpty = document.getElementById("stream-empty");
      if (streamText && streamEmpty) {
        streamEmpty.style.display = "none";
        streamText.style.display = "";
        streamText.textContent = data.report || "";
      }
    } catch (err) {
      // silently fail
    }
  }

  // ── Click handlers ──
  document.addEventListener("click", (e) => {
    // Swarm suggestions
    const swarmSug = e.target.closest(".swarm-suggestion");
    if (swarmSug) {
      const query = swarmSug.dataset.query;
      if (query) launchSwarmWithQuery(query);
      return;
    }
    // History items
    const histItem = e.target.closest(".history-item");
    if (histItem) {
      const rid = histItem.dataset.reportId;
      if (rid) viewReport(rid);
      return;
    }
    // Chat suggestions
    if (e.target.classList.contains("suggestion")) {
      userInput.value = e.target.dataset.msg;
      sendBtn.disabled = false;
      sendMessage();
    }
  });

  // ── Send chat message ──
  async function sendMessage() {
    const text = userInput.value.trim();
    if (!text || isStreaming) return;

    if (welcomeEl) welcomeEl.remove();

    conversationHistory.push({ role: "user", content: text });
    appendUserMessage(text);

    userInput.value = "";
    userInput.style.height = "auto";
    sendBtn.disabled = true;
    isStreaming = true;

    const { messageEl, thinkingBlock, contentEl } = appendAssistantPlaceholder();
    scrollToBottom();

    try {
      const response = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ messages: conversationHistory }),
      });

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      let fullContent = "";
      let fullThinking = "";
      let rawStream = "";   // accumulates raw text for <think> tag parsing
      let insideThink = false;

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.startsWith("data:")) continue;
          const dataStr = line.slice(5).trim();
          if (dataStr === "[DONE]") continue;

          try {
            const data = JSON.parse(dataStr);
            const choice = data.choices?.[0];
            if (!choice) continue;

            const delta = choice.delta || {};

            // Handle reasoning_content field (OpenAI-style thinking)
            if (delta.reasoning_content) {
              fullThinking += delta.reasoning_content;
              updateThinkingBlock(thinkingBlock, fullThinking);
            }

            if (delta.content) {
              // MiniMax M2.5 embeds thinking in <think>...</think> tags within content
              rawStream += delta.content;

              // Parse <think> tags from accumulated stream
              let visibleContent = "";
              let thinkContent = "";
              let temp = rawStream;
              // Extract completed <think>...</think> blocks
              const thinkRegex = /<think>([\s\S]*?)<\/think>/g;
              let match;
              let lastEnd = 0;
              while ((match = thinkRegex.exec(temp)) !== null) {
                visibleContent += temp.slice(lastEnd, match.index);
                thinkContent += match[1];
                lastEnd = match.index + match[0].length;
              }
              const remainder = temp.slice(lastEnd);

              // Check if we're inside an unclosed <think> tag
              const openIdx = remainder.lastIndexOf("<think>");
              if (openIdx !== -1) {
                visibleContent += remainder.slice(0, openIdx);
                thinkContent += remainder.slice(openIdx + 7); // after <think>
                insideThink = true;
              } else {
                insideThink = false;
                visibleContent += remainder;
              }

              // Update thinking block if we have thinking content
              if (thinkContent.trim()) {
                fullThinking = thinkContent;
                updateThinkingBlock(thinkingBlock, fullThinking);
              }

              // Update visible content
              fullContent = visibleContent.trim();
              if (fullContent) {
                // safe: renderMarkdown calls escapeHtml on ALL input before applying transforms
                contentEl.innerHTML = renderMarkdown(fullContent);
              }
            }

            scrollToBottom();
          } catch {
            // skip non-JSON lines
          }
        }
      }

      if (fullContent) {
        conversationHistory.push({ role: "assistant", content: fullContent });
      }

      const typing = messageEl.querySelector(".typing-indicator");
      if (typing) typing.remove();

      if (!fullContent && !fullThinking) {
        // safe: renderMarkdown escapes all input via escapeHtml first
        contentEl.innerHTML = renderMarkdown("*No response received. Please try again.*");
      }
    } catch (err) {
      // safe: renderMarkdown escapes all input via escapeHtml first
      contentEl.innerHTML = renderMarkdown("**Connection error:** " + err.message);
    }

    isStreaming = false;
    sendBtn.disabled = userInput.value.trim() === "";
    userInput.focus();
  }

  // ── Append user message ──
  function appendUserMessage(text) {
    const msg = document.createElement("div");
    msg.className = "message user";

    const avatar = document.createElement("div");
    avatar.className = "message-avatar";
    avatar.textContent = "You";

    const body = document.createElement("div");
    body.className = "message-body";

    const content = document.createElement("div");
    content.className = "message-content";
    // safe: renderMarkdown escapes all input via escapeHtml before applying transforms
    content.innerHTML = renderMarkdown(text);

    body.appendChild(content);
    msg.appendChild(avatar);
    msg.appendChild(body);
    messagesEl.appendChild(msg);
    scrollToBottom();
  }

  // ── Append assistant placeholder ──
  function appendAssistantPlaceholder() {
    const msg = document.createElement("div");
    msg.className = "message assistant";

    const avatar = document.createElement("div");
    avatar.className = "message-avatar";
    avatar.textContent = "A";

    const thinkingBlock = document.createElement("div");
    thinkingBlock.className = "thinking-block";
    thinkingBlock.style.display = "none";

    const contentEl = document.createElement("div");
    contentEl.className = "message-content";

    const typing = document.createElement("div");
    typing.className = "typing-indicator";
    for (let i = 0; i < 3; i++) {
      const dot = document.createElement("div");
      dot.className = "typing-dot";
      typing.appendChild(dot);
    }
    contentEl.appendChild(typing);

    const body = document.createElement("div");
    body.className = "message-body";
    body.appendChild(thinkingBlock);
    body.appendChild(contentEl);

    msg.appendChild(avatar);
    msg.appendChild(body);
    messagesEl.appendChild(msg);

    return { messageEl: msg, thinkingBlock, contentEl };
  }

  // ── Update thinking block ──
  function updateThinkingBlock(block, text) {
    block.style.display = "block";

    if (!block.querySelector(".thinking-toggle")) {
      const toggle = document.createElement("button");
      toggle.className = "thinking-toggle";

      const arrow = document.createElement("span");
      arrow.className = "arrow";
      arrow.textContent = "\u25B6";
      const label = document.createElement("span");
      label.textContent = " Thinking\u2026";
      toggle.appendChild(arrow);
      toggle.appendChild(label);

      const content = document.createElement("div");
      content.className = "thinking-content";

      const textEl = document.createElement("div");
      textEl.className = "thinking-text";

      content.appendChild(textEl);
      block.appendChild(toggle);
      block.appendChild(content);

      toggle.addEventListener("click", () => {
        toggle.classList.toggle("open");
        content.classList.toggle("open");
      });
    }

    const textEl = block.querySelector(".thinking-text");
    // safe: renderMarkdown escapes all input via escapeHtml before applying formatting
    textEl.innerHTML = renderMarkdown(text);
  }

  // NOTE: escapeHtml is called before any HTML transforms in renderMarkdown,
  // ensuring user-provided text is sanitized before being inserted into the DOM.
  function escapeHtml(str) {
    const div = document.createElement("div");
    div.textContent = str;
    return div.innerHTML;
  }

  // renderMarkdown escapes ALL input via escapeHtml first, then applies
  // safe formatting transforms on the already-escaped string.
  function renderMarkdown(text) {
    if (!text) return "";
    let html = escapeHtml(text);

    html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
      return '<pre><code class="language-' + lang + '">' + code.trim() + "</code></pre>";
    });
    html = html.replace(/`([^`]+)`/g, "<code>$1</code>");
    // Tables: consecutive lines starting/ending with |
    html = html.replace(/(^\|.+\|[ \t]*$\n?)+/gm, function(block) {
      var rows = block.trim().split('\n');
      if (rows.length < 2) return block;
      if (!/^\|[\s\-:]+\|$/.test(rows[1])) return block;
      var pr = function(r) { return r.split('|').slice(1, -1).map(function(c) { return c.trim(); }); };
      var hds = pr(rows[0]);
      var t = '<table><thead><tr>' + hds.map(function(c) { return '<th>' + c + '</th>'; }).join('') + '</tr></thead><tbody>';
      for (var i = 2; i < rows.length; i++) {
        var cells = pr(rows[i]);
        t += '<tr>' + cells.map(function(c) { return '<td>' + c + '</td>'; }).join('') + '</tr>';
      }
      return t + '</tbody></table>';
    });
    // Blockquotes (> is escaped to &gt; by escapeHtml)
    html = html.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>');
    html = html.replace(/<\/blockquote>\n<blockquote>/g, '<br/>');
    // Horizontal rules
    html = html.replace(/^-{3,}$/gm, '<hr/>');
    html = html.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    html = html.replace(/\*(.+?)\*/g, "<em>$1</em>");
    html = html.replace(/^### (.+)$/gm, "<h4>$1</h4>");
    html = html.replace(/^## (.+)$/gm, "<h3>$1</h3>");
    html = html.replace(/^# (.+)$/gm, "<h2>$1</h2>");
    html = html.replace(/^\s*[-*] (.+)$/gm, "<li>$1</li>");
    html = html.replace(/(<li>.*<\/li>\n?)+/g, (match) => "<ul>" + match + "</ul>");
    html = html.replace(/^\s*\d+\. (.+)$/gm, "<li>$1</li>");
    html = html.replace(/\n\n/g, "</p><p>");
    html = html.replace(/\n/g, "<br/>");
    if (!html.startsWith("<")) {
      html = "<p>" + html + "</p>";
    }
    return html;
  }

  function scrollToBottom() {
    requestAnimationFrame(() => {
      if (chatContainer) chatContainer.scrollTop = chatContainer.scrollHeight;
    });
  }

  // Close report overlay on Escape
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const overlay = document.getElementById('report-overlay');
      if (overlay && overlay.classList.contains('active')) {
        overlay.classList.remove('active');
      }
    }
  });
})();
