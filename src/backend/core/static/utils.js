// Shared utility functions used by both app.js and swarm.js

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
