pub fn render_portal_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>CITADEL — Secure Offline Assessment Portal</title>
  <style>
    :root {
      --bg-base: #06090f;
      --bg-surface: #0c121e;
      --bg-surface-elevated: #131b2e;
      --bg-editor: #080d16;
      --border-subtle: rgba(255, 255, 255, 0.08);
      --border-focus: #3b82f6;
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      --text-dim: #64748b;
      --accent-blue: #3b82f6;
      --accent-cyan: #06b6d4;
      --accent-green: #10b981;
      --accent-amber: #f59e0b;
      --accent-red: #ef4444;
      --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; user-select: none; }

    body {
      background-color: var(--bg-base);
      background-image: 
        radial-gradient(circle at 10% 10%, rgba(37, 99, 235, 0.08) 0%, transparent 45%),
        radial-gradient(circle at 90% 90%, rgba(6, 182, 212, 0.06) 0%, transparent 45%);
      color: var(--text-main);
      font-family: var(--font-ui);
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      -webkit-font-smoothing: antialiased;
    }

    
    /* SECURITY TOAST NOTIFICATION */
    #security-toast {
      display: none;
      position: fixed;
      top: 50px;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(220, 38, 38, 0.95);
      backdrop-filter: blur(12px);
      color: white;
      padding: 12px 24px;
      border-radius: 8px;
      font-size: 14px;
      font-weight: 600;
      box-shadow: 0 10px 25px rgba(0, 0, 0, 0.6);
      z-index: 999999;
      border: 1px solid rgba(248, 113, 113, 0.5);
      animation: slideDown 0.25s ease-out;
    }
    @keyframes slideDown {
      from { transform: translate(-50%, -20px); opacity: 0; }
      to { transform: translate(-50%, 0); opacity: 1; }
    }

    /* SECURITY BLUR ALERT BANNER */
    #blur-warning {
      display: none;
      position: fixed;
      top: 0; left: 0; right: 0;
      background: linear-gradient(90deg, #dc2626, #991b1b);
      color: white;
      font-weight: 700;
      font-size: 13px;
      padding: 8px 16px;
      text-align: center;
      z-index: 10000;
      box-shadow: 0 4px 20px rgba(220, 38, 38, 0.6);
      animation: flash 0.5s infinite alternate;
    }
    @keyframes flash {
      from { opacity: 0.9; }
      to { opacity: 1; }
    }

    /* TOP HEADER */
    header {
      background: rgba(12, 18, 30, 0.85);
      backdrop-filter: blur(20px);
      -webkit-backdrop-filter: blur(20px);
      border-bottom: 1px solid var(--border-subtle);
      padding: 0 20px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      height: 56px;
      flex-shrink: 0;
      z-index: 50;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .brand-logo {
      background: linear-gradient(135deg, #2563eb, #06b6d4);
      color: white;
      font-weight: 900;
      font-size: 13px;
      letter-spacing: 1.5px;
      padding: 5px 10px;
      border-radius: 8px;
      box-shadow: 0 0 16px rgba(37, 99, 235, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.2);
    }

    .brand-title {
      font-size: 14px;
      font-weight: 700;
      letter-spacing: 0.5px;
      color: var(--text-main);
    }

    .brand-subtitle {
      font-size: 11px;
      color: var(--text-dim);
      padding-left: 10px;
      border-left: 1px solid var(--border-subtle);
      font-family: var(--font-mono);
    }

    .header-center {
      display: flex;
      align-items: center;
      gap: 16px;
    }

    .status-badge {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      background: rgba(16, 185, 129, 0.08);
      border: 1px solid rgba(16, 185, 129, 0.25);
      color: #34d399;
      padding: 5px 12px;
      border-radius: 9999px;
      font-size: 12px;
      font-weight: 600;
      letter-spacing: 0.3px;
    }

    .beacon {
      position: relative;
      display: flex;
      width: 8px;
      height: 8px;
    }

    .beacon-ping {
      position: absolute;
      display: inline-flex;
      height: 100%;
      width: 100%;
      border-radius: 9999px;
      background-color: var(--accent-green);
      opacity: 0.75;
      animation: ping 1.5s cubic-bezier(0, 0, 0.2, 1) infinite;
    }

    .beacon-dot {
      position: relative;
      display: inline-flex;
      border-radius: 9999px;
      height: 8px;
      width: 8px;
      background-color: var(--accent-green);
      box-shadow: 0 0 8px var(--accent-green);
    }

    @keyframes ping {
      75%, 100% {
        transform: scale(2.2);
        opacity: 0;
      }
    }

    .timer-badge {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      background: rgba(245, 158, 11, 0.08);
      border: 1px solid rgba(245, 158, 11, 0.25);
      color: #fbbf24;
      padding: 5px 14px;
      border-radius: 8px;
      font-family: var(--font-mono);
      font-size: 13px;
      font-weight: 700;
      letter-spacing: 1px;
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .candidate-pill {
      font-size: 11px;
      color: var(--text-dim);
      font-family: var(--font-mono);
      background: rgba(255, 255, 255, 0.03);
      padding: 4px 8px;
      border-radius: 6px;
      border: 1px solid var(--border-subtle);
    }

    .btn-finish {
      background: linear-gradient(180deg, #dc2626 0%, #b91c1c 100%);
      color: white;
      border: 1px solid rgba(255, 255, 255, 0.15);
      padding: 6px 14px;
      border-radius: 6px;
      font-weight: 600;
      font-size: 12px;
      cursor: pointer;
      box-shadow: 0 2px 8px rgba(220, 38, 38, 0.3);
      transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    }
    .btn-finish:hover {
      transform: translateY(-1px);
      box-shadow: 0 4px 12px rgba(220, 38, 38, 0.5);
    }

    /* MAIN CONTAINER */
    .workspace {
      display: flex;
      flex: 1;
      height: calc(100vh - 56px);
      overflow: hidden;
    }

    /* LEFT PANE: QUESTIONS */
    .pane-left {
      width: 48%;
      border-right: 1px solid var(--border-subtle);
      background: var(--bg-surface);
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }

    .q-tabs {
      display: flex;
      border-bottom: 1px solid var(--border-subtle);
      background: rgba(6, 9, 15, 0.9);
      padding: 6px 12px 0 12px;
      gap: 6px;
    }

    .q-tab {
      padding: 8px 16px;
      font-size: 12px;
      font-weight: 600;
      color: var(--text-dim);
      cursor: pointer;
      border-radius: 6px 6px 0 0;
      border: 1px solid transparent;
      border-bottom: none;
      transition: all 0.15s;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .q-tab:hover {
      color: var(--text-main);
      background: rgba(255, 255, 255, 0.03);
    }

    .q-tab.active {
      color: #60a5fa;
      background: var(--bg-surface);
      border-color: var(--border-subtle);
      box-shadow: inset 0 2px 0 var(--accent-blue);
    }

    .tab-status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--text-dim);
    }
    .q-tab.active .tab-status-dot { background: var(--accent-blue); }
    .tab-status-dot.solved { background: var(--accent-green) !important; box-shadow: 0 0 6px var(--accent-green); }

    .q-content {
      padding: 24px 28px;
      overflow-y: auto;
      flex: 1;
      user-select: text;
    }

    .q-title {
      font-size: 20px;
      font-weight: 700;
      letter-spacing: -0.01em;
      margin-bottom: 12px;
      color: var(--text-main);
    }

    .q-meta {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 24px;
    }

    .pill {
      font-size: 11px;
      padding: 3px 10px;
      border-radius: 9999px;
      font-weight: 600;
      letter-spacing: 0.3px;
    }

    .pill-diff { background: rgba(59, 130, 246, 0.12); color: #93c5fd; border: 1px solid rgba(59, 130, 246, 0.25); }
    .pill-pts { background: rgba(16, 185, 129, 0.12); color: #6ee7b7; border: 1px solid rgba(16, 185, 129, 0.25); }
    .pill-tag { background: rgba(255, 255, 255, 0.04); color: var(--text-muted); border: 1px solid var(--border-subtle); }

    .q-body {
      font-size: 14px;
      line-height: 1.7;
      color: #cbd5e1;
      margin-bottom: 24px;
      white-space: pre-line;
    }

    .section-title {
      font-size: 12px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.8px;
      color: var(--text-muted);
      margin-bottom: 10px;
    }

    .constraints-list {
      margin-left: 20px;
      margin-bottom: 24px;
      font-size: 13px;
      color: #94a3b8;
    }
    .constraints-list li { margin-bottom: 6px; font-family: var(--font-mono); }

    .sample-card {
      background: var(--bg-base);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 14px;
      margin-bottom: 16px;
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
    }

    .sample-label {
      font-size: 11px;
      font-weight: 700;
      color: var(--text-dim);
      margin-bottom: 6px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .sample-box {
      background: #04060a;
      border: 1px solid rgba(255, 255, 255, 0.05);
      padding: 10px 14px;
      border-radius: 6px;
      font-family: var(--font-mono);
      font-size: 12px;
      color: #38bdf8;
      margin-bottom: 10px;
      white-space: pre-wrap;
    }

    /* RIGHT PANE: CODE EDITOR */
    .pane-right {
      width: 52%;
      display: flex;
      flex-direction: column;
      background: var(--bg-editor);
    }

    .editor-toolbar {
      background: rgba(12, 18, 30, 0.95);
      border-bottom: 1px solid var(--border-subtle);
      padding: 8px 16px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      height: 48px;
      flex-shrink: 0;
    }

    .lang-select {
      background: var(--bg-base);
      color: var(--text-main);
      border: 1px solid var(--border-subtle);
      padding: 6px 12px;
      border-radius: 6px;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      outline: none;
      transition: border-color 0.15s;
    }
    .lang-select:focus { border-color: var(--border-focus); }

    .file-badge {
      font-size: 11px;
      color: var(--text-dim);
      font-family: var(--font-mono);
      padding: 2px 6px;
      background: rgba(255, 255, 255, 0.02);
      border-radius: 4px;
    }

    .editor-container {
      flex: 1;
      position: relative;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }

    .code-area {
      flex: 1;
      background: #070b12;
      color: #e2e8f0;
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.6;
      padding: 18px 20px;
      border: none;
      outline: none;
      resize: none;
      white-space: pre;
      tab-size: 4;
      user-select: text;
    }

    /* BOTTOM RESULTS PANEL */
    .results-panel {
      height: 220px;
      border-top: 1px solid var(--border-subtle);
      background: var(--bg-surface);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
    }

    .results-header {
      padding: 8px 16px;
      background: var(--bg-base);
      border-bottom: 1px solid var(--border-subtle);
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .results-title {
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.8px;
      color: var(--text-muted);
    }

    .editor-btn-group {
      display: flex;
      gap: 10px;
    }

    .btn {
      padding: 7px 16px;
      border-radius: 6px;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      border: 1px solid transparent;
      transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    }

    .btn-secondary {
      background: rgba(255, 255, 255, 0.04);
      color: var(--text-main);
      border: 1px solid var(--border-subtle);
    }
    .btn-secondary:hover {
      background: rgba(255, 255, 255, 0.08);
      border-color: rgba(255, 255, 255, 0.15);
    }

    .btn-primary {
      background: linear-gradient(180deg, #3b82f6 0%, #1d4ed8 100%);
      color: white;
      box-shadow: 0 0 16px rgba(59, 130, 246, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
    }
    .btn-primary:hover {
      box-shadow: 0 0 24px rgba(59, 130, 246, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.3);
      transform: translateY(-1px);
    }

    .btn-success {
      background: linear-gradient(180deg, #10b981 0%, #047857 100%);
      color: white;
      box-shadow: 0 0 16px rgba(16, 185, 129, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
    }
    .btn-success:hover {
      box-shadow: 0 0 24px rgba(16, 185, 129, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.3);
      transform: translateY(-1px);
    }

    .results-body {
      flex: 1;
      padding: 14px 18px;
      overflow-y: auto;
      font-family: var(--font-mono);
      font-size: 12px;
      user-select: text;
    }

    .verdict-tag {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 12px;
      border-radius: 9999px;
      font-weight: 700;
      font-size: 11px;
      letter-spacing: 0.5px;
      margin-bottom: 10px;
    }
    .verdict-passed { background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }
    .verdict-running { background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.3); }

    .stat-pill {
      font-size: 11px;
      color: var(--text-dim);
      font-family: var(--font-mono);
      margin-left: 12px;
    }
  </style>
</head>
<body>

  <!-- FOCUS LOSS WARNING MODAL -->
  <div id="security-toast">⚠️ <span id="security-toast-msg">Security Violation</span></div>
  <div id="blur-warning">
    ⚠️ SECURITY WARNING: Window focus lost! Focus-loss events are recorded in the exam integrity log.
  </div>

  <!-- TOP HEADER -->
  <header>
    <div class="brand">
      <span class="brand-logo">CITADEL</span>
      <span class="brand-title">CAMPUS PLACEMENT ASSESSMENT 2026</span>
      <span class="brand-subtitle" id="server-ip-banner">Local Wi-Fi Host</span>
    </div>

    <div class="header-center">
      <div class="status-badge">
        <span class="beacon">
          <span class="beacon-ping"></span>
          <span class="beacon-dot"></span>
        </span>
        <span>Secure College Wi-Fi • Air-Gapped</span>
      </div>
      <div class="timer-badge" id="exam-timer">01:29:54</div>
    </div>

    <div class="header-actions">
      <span class="candidate-pill" id="candidate-id">BYOD-STATION</span>
      <button class="btn-finish" onclick="finishExam()">Finish Exam</button>
    </div>
  </header>

  <!-- WORKSPACE -->
  <div class="workspace">
    <!-- LEFT PANE: QUESTIONS -->
    <div class="pane-left">
      <div class="q-tabs" id="question-tabs">
        <!-- Rendered dynamically -->
      </div>
      <div class="q-content" id="question-view">
        <!-- Rendered dynamically -->
      </div>
    </div>

    <!-- RIGHT PANE: CODE EDITOR -->
    <div class="pane-right">
      <div class="editor-toolbar">
        <div style="display: flex; align-items: center; gap: 12px;">
          <select class="lang-select" id="lang-selector" onchange="onLanguageChange()">
            <option value="python">Python 3 (3.11)</option>
            <option value="cpp">C++ (GCC 17)</option>
            <option value="java">Java (OpenJDK 17)</option>
          </select>
          <span class="file-badge" id="file-label">solution.py</span>
        </div>

        <div class="editor-btn-group">
          <button class="btn btn-secondary" onclick="resetStarterCode()">Reset Code</button>
          <button class="btn btn-primary" onclick="runSampleTests()">Run Sample Tests</button>
          <button class="btn btn-success" onclick="submitFinalSolution()">Submit Solution</button>
        </div>
      </div>

      <div class="editor-container">
        <textarea class="code-area" id="code-editor" spellcheck="false" placeholder="Write your solution here..."></textarea>
      </div>

      <!-- RESULTS PANEL -->
      <div class="results-panel">
        <div class="results-header">
          <span class="results-title">Sandbox Execution Output & Test Verification</span>
          <span id="exec-stats" class="stat-pill">Isolated Local Sandbox • Ready</span>
        </div>
        <div class="results-body" id="results-console">
          <div style="color: var(--text-dim); line-height: 1.6;">Click 'Run Sample Tests' to compile and execute your code against test cases in the offline evaluation sandbox.</div>
        </div>
      </div>
    </div>
  </div>

  <script>
    let questions = [];
    let currentQIndex = 0;
    let currentLang = 'python';
    let codeStorage = {};

    // =========================================================================
    // IN-BROWSER SECURITY & HARDENING
    // =========================================================================

    // 1. Disable Right-Click Context Menu / Inspect Element
    document.addEventListener('contextmenu', e => {
      e.preventDefault();
      return false;
    });

    function showSecurityToast(msg) {
      const toast = document.getElementById('security-toast');
      const text = document.getElementById('security-toast-msg');
      if (toast && text) {
        text.innerText = msg;
        toast.style.display = 'block';
        clearTimeout(window._toastTimer);
        window._toastTimer = setTimeout(() => {
          toast.style.display = 'none';
        }, 4000);
      }
    }

    // 2. Disable Copying Exam Content to External Clipboard
    document.addEventListener('copy', e => {
      if (document.activeElement && document.activeElement.id === 'code-editor') {
        // Allowed inside editor for candidate's own code
      } else {
        e.preventDefault();
        showSecurityToast('Copying exam questions or instructions is strictly prohibited.');
      }
    });

    // 3. Disable Pasting External Code
    document.addEventListener('paste', e => {
      e.preventDefault();
      showSecurityToast('Pasting external content is blocked. Please write your code manually.');
    });

    // 4. Disable DevTools Shortcuts (F12, Ctrl+Shift+I, Ctrl+Shift+J, Ctrl+U)
    document.addEventListener('keydown', e => {
      if (
        e.key === 'F12' ||
        (e.ctrlKey && e.shiftKey && (e.key === 'I' || e.key === 'i' || e.key === 'J' || e.key === 'j')) ||
        (e.ctrlKey && (e.key === 'u' || e.key === 'U'))
      ) {
        e.preventDefault();
        return false;
      }
    });

    // 5. Monitor Window Blur (Focus-Loss / Window Switching)
    window.addEventListener('blur', () => {
      const banner = document.getElementById('blur-warning');
      if (banner) banner.style.display = 'block';
    });
    window.addEventListener('focus', () => {
      setTimeout(() => {
        const banner = document.getElementById('blur-warning');
        if (banner) banner.style.display = 'none';
      }, 3000);
    });

    // =========================================================================
    // EXAM LOGIC & COMMUNICATION
    // =========================================================================

    async function init() {
      try {
        const res = await fetch('/api/v1/questions');
        questions = await res.json();
        renderTabs();
        loadQuestion(0);
        startTimer();
      } catch (err) {
        console.error('Failed to load questions:', err);
      }
    }

    function renderTabs() {
      const container = document.getElementById('question-tabs');
      container.innerHTML = questions.map((q, idx) => `
        <div class="q-tab ${idx === 0 ? 'active' : ''}" onclick="loadQuestion(${idx})" id="tab-${idx}">
          <span class="tab-status-dot" id="dot-${q.id}"></span>
          <span>Q${q.number}. ${q.title.split(' ')[0]}</span>
          <span style="font-size: 10px; opacity: 0.6; font-family: var(--font-mono);">(${q.points}p)</span>
        </div>
      `).join('');
    }

    async function loadQuestion(idx) {
      currentQIndex = idx;
      document.querySelectorAll('.q-tab').forEach((el, i) => {
        el.classList.toggle('active', i === idx);
      });

      const qSummary = questions[idx];
      const res = await fetch(`/api/v1/questions/${qSummary.id}`);
      const q = await res.json();

      const view = document.getElementById('question-view');
      view.innerHTML = `
        <h1 class="q-title">Q${q.number}. ${q.title}</h1>
        <div class="q-meta">
          <span class="pill pill-diff">${q.difficulty}</span>
          <span class="pill pill-pts">${q.points} Points</span>
          ${q.tags.map(t => `<span class="pill pill-tag">${t}</span>`).join('')}
        </div>

        <div class="section-title">Problem Statement</div>
        <div class="q-body">${q.description}</div>

        <div class="section-title">Input Format</div>
        <div class="q-body" style="margin-bottom: 16px;">${q.input_format}</div>

        <div class="section-title">Output Format</div>
        <div class="q-body" style="margin-bottom: 16px;">${q.output_format}</div>

        <div class="section-title">Constraints</div>
        <ul class="constraints-list">
          ${q.constraints.map(c => `<li>${c}</li>`).join('')}
        </ul>

        <div class="section-title">Sample Test Cases</div>
        ${q.sample_cases.map((sc, i) => `
          <div class="sample-card">
            <div class="sample-label">Sample Input ${i + 1}</div>
            <div class="sample-box">${sc.input}</div>
            <div class="sample-label">Sample Output ${i + 1}</div>
            <div class="sample-box" style="color: #34d399;">${sc.expected_output}</div>
            ${sc.explanation ? `<div style="font-size: 11px; color: var(--text-dim); margin-top: 6px; line-height: 1.5;"><strong>Explanation:</strong> ${sc.explanation}</div>` : ''}
          </div>
        `).join('')}
      `;

      updateEditorForCurrentQuestion(q);
    }

    function updateEditorForCurrentQuestion(q) {
      const key = `${q.id}_${currentLang}`;
      const editor = document.getElementById('code-editor');
      if (codeStorage[key]) {
        editor.value = codeStorage[key];
      } else if (q.starter_templates && q.starter_templates[currentLang]) {
        editor.value = q.starter_templates[currentLang];
      } else {
        editor.value = '';
      }
      updateFileLabel();
    }

    function onLanguageChange() {
      saveCurrentCode();
      currentLang = document.getElementById('lang-selector').value;
      const q = questions[currentQIndex];
      if (q) {
        fetch(`/api/v1/questions/${q.id}`)
          .then(res => res.json())
          .then(fullQ => updateEditorForCurrentQuestion(fullQ));
      }
    }

    function saveCurrentCode() {
      const q = questions[currentQIndex];
      if (q) {
        const key = `${q.id}_${currentLang}`;
        codeStorage[key] = document.getElementById('code-editor').value;
      }
    }

    function updateFileLabel() {
      const extMap = { python: 'solution.py', cpp: 'solution.cpp', java: 'Solution.java' };
      document.getElementById('file-label').innerText = extMap[currentLang] || 'solution.txt';
    }

    async function resetStarterCode() {
      const q = questions[currentQIndex];
      if (confirm(`Reset code for ${q.title} to default template?`)) {
        const res = await fetch(`/api/v1/questions/${q.id}`);
        const fullQ = await res.json();
        const key = `${q.id}_${currentLang}`;
        delete codeStorage[key];
        updateEditorForCurrentQuestion(fullQ);
      }
    }

    async function runSampleTests() {
      saveCurrentCode();
      const q = questions[currentQIndex];
      const code = document.getElementById('code-editor').value;
      const consoleEl = document.getElementById('results-console');
      const statsEl = document.getElementById('exec-stats');

      consoleEl.innerHTML = `<div class="verdict-tag verdict-running">RUNNING SAMPLE TESTS...</div><div style="color: var(--text-dim);">Dispatching code to local offline sandbox...</div>`;
      statsEl.innerText = "Evaluating...";

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: q.id,
            language: currentLang,
            source_code: code,
            is_sample_run: true,
          })
        });

        const data = await res.json();
        const passed = data.status === 'Accepted';
        statsEl.innerText = `Runtime: ${data.runtime_ms}ms • Memory: ${data.memory_mb}MB`;

        if (passed) {
          const dot = document.getElementById(`dot-${q.id}`);
          if (dot) dot.classList.add('solved');
        }

        consoleEl.innerHTML = `
          <div class="verdict-tag ${passed ? 'verdict-passed' : 'verdict-running'}" style="${passed ? '' : 'background: rgba(239, 68, 68, 0.15); color: #f87171; border-color: rgba(239, 68, 68, 0.3);'}">
            ${data.status.toUpperCase()} (${data.passed_cases}/${data.total_cases} Sample Tests Passed)
          </div>
          <div style="color: #94a3b8; margin-top: 6px; white-space: pre-wrap; line-height: 1.5;">${data.details}</div>
        `;
      } catch (err) {
        consoleEl.innerHTML = `<div style="color: #f87171;">Failed to connect to local server: ${err.message}</div>`;
      }
    }

    async function submitFinalSolution() {
      saveCurrentCode();
      const q = questions[currentQIndex];
      const code = document.getElementById('code-editor').value;
      const consoleEl = document.getElementById('results-console');

      if (!confirm(`Confirm final submission for Q${q.number}: ${q.title}?`)) return;

      consoleEl.innerHTML = `<div class="verdict-tag verdict-running">SUBMITTING FOR EVALUATION...</div>`;

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: q.id,
            language: currentLang,
            source_code: code,
            is_sample_run: false,
          })
        });

        const data = await res.json();
        consoleEl.innerHTML = `
          <div class="verdict-tag verdict-passed">
            SUBMISSION RECORDED — SCORE: ${data.score}/${q.points} PTS
          </div>
          <div style="color: #94a3b8; margin-top: 6px;">Your submission has been securely written to the appliance audit WAL.</div>
        `;
      } catch (err) {
        consoleEl.innerHTML = `<div style="color: #f87171;">Submission failed: ${err.message}</div>`;
      }
    }

    function finishExam() {
      if (confirm("Are you sure you want to end and submit your entire assessment?")) {
        alert("Assessment successfully submitted! You may now exit the lockdown app.");
      }
    }

    function startTimer() {
      let seconds = 90 * 60 - 6;
      setInterval(() => {
        if (seconds > 0) seconds--;
        const h = String(Math.floor(seconds / 3600)).padStart(2, '0');
        const m = String(Math.floor((seconds % 3600) / 60)).padStart(2, '0');
        const s = String(seconds % 60).padStart(2, '0');
        document.getElementById('exam-timer').innerText = `${h}:${m}:${s}`;
      }, 1000);
    }

    document.getElementById('code-editor').addEventListener('keydown', function(e) {
      if (e.key === 'Tab') {
        e.preventDefault();
        const start = this.selectionStart;
        const end = this.selectionEnd;
        this.value = this.value.substring(0, start) + "    " + this.value.substring(end);
        this.selectionStart = this.selectionEnd = start + 4;
      }
    });

    init();
  </script>
</body>
</html>
"#
}


pub fn render_gatekeeper_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>CITADEL — Client Required</title>
  <style>
    :root {
      --bg-base: #06090f;
      --bg-surface: #0c121e;
      --bg-surface-elevated: #131b2e;
      --border-subtle: rgba(255, 255, 255, 0.08);
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      --text-dim: #64748b;
      --accent-blue: #3b82f6;
      --accent-cyan: #06b6d4;
      --accent-red: #ef4444;
      --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      background: var(--bg-base);
      color: var(--text-main);
      font-family: var(--font-ui);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 24px;
      background-image: 
        radial-gradient(circle at 50% 15%, rgba(59, 130, 246, 0.12), transparent 45%),
        radial-gradient(circle at 80% 80%, rgba(6, 182, 212, 0.08), transparent 40%);
    }

    .container {
      max-width: 680px;
      width: 100%;
      background: var(--bg-surface);
      border: 1px solid var(--border-subtle);
      border-radius: 16px;
      padding: 40px;
      box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.6);
      text-align: center;
      position: relative;
      overflow: hidden;
    }

    .container::before {
      content: '';
      position: absolute;
      top: 0; left: 0; right: 0;
      height: 3px;
      background: linear-gradient(90deg, #3b82f6, #06b6d4, #3b82f6);
    }

    .shield-badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 64px;
      height: 64px;
      background: rgba(59, 130, 246, 0.1);
      border: 1px solid rgba(59, 130, 246, 0.3);
      border-radius: 50%;
      font-size: 28px;
      margin-bottom: 20px;
      box-shadow: 0 0 20px rgba(59, 130, 246, 0.2);
    }

    .badge-pill {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 12px;
      background: rgba(239, 68, 68, 0.15);
      border: 1px solid rgba(239, 68, 68, 0.3);
      color: #f87171;
      border-radius: 20px;
      font-size: 12px;
      font-weight: 600;
      letter-spacing: 0.5px;
      text-transform: uppercase;
      margin-bottom: 16px;
    }

    .badge-pill .dot {
      width: 6px;
      height: 6px;
      background: #ef4444;
      border-radius: 50%;
      animation: pulse 1.5s infinite;
    }

    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(0.85); }
    }

    h1 {
      font-size: 26px;
      font-weight: 700;
      margin-bottom: 12px;
      letter-spacing: -0.5px;
    }

    p.lead {
      color: var(--text-muted);
      font-size: 15px;
      line-height: 1.6;
      margin-bottom: 28px;
    }

    .download-card {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 12px;
      padding: 24px;
      margin-bottom: 28px;
    }

    .btn-download {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 10px;
      background: linear-gradient(135deg, #2563eb, #1d4ed8);
      color: white;
      text-decoration: none;
      padding: 14px 28px;
      border-radius: 10px;
      font-weight: 600;
      font-size: 15px;
      box-shadow: 0 4px 14px rgba(37, 99, 235, 0.4);
      transition: all 0.2s ease;
      width: 100%;
      cursor: pointer;
    }

    .btn-download:hover {
      background: linear-gradient(135deg, #3b82f6, #2563eb);
      box-shadow: 0 6px 20px rgba(37, 99, 235, 0.6);
      transform: translateY(-1px);
    }

    .file-meta {
      font-size: 12px;
      color: var(--text-dim);
      margin-top: 10px;
    }

    .steps-container {
      text-align: left;
      margin-bottom: 24px;
    }

    .steps-title {
      font-size: 13px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: var(--text-muted);
      margin-bottom: 12px;
    }

    .step-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      margin-bottom: 12px;
      background: rgba(255, 255, 255, 0.02);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 12px 14px;
    }

    .step-number {
      width: 24px;
      height: 24px;
      background: #1e293b;
      color: var(--accent-cyan);
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 12px;
      font-weight: 700;
      flex-shrink: 0;
    }

    .step-text {
      font-size: 13px;
      color: var(--text-muted);
      line-height: 1.4;
    }

    .step-text strong {
      color: var(--text-main);
    }

    .footer-note {
      font-size: 12px;
      color: var(--text-dim);
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
    }

    .footer-note .tag {
      background: #1e293b;
      padding: 2px 6px;
      border-radius: 4px;
      font-family: var(--font-mono);
      font-size: 11px;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="shield-badge">🛡️</div>
    <div class="badge-pill">
      <span class="dot"></span>
      Lockdown Required
    </div>
    <h1>CITADEL Assessment Environment</h1>
    <p class="lead">
      To ensure examination integrity, questions cannot be accessed through standard web browsers or mobile devices. 
      Please download and run the elevated CITADEL Client to begin your assessment.
    </p>

    <div class="download-card">
      <a href="/download/citadel-client.exe" class="btn-download">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="7 10 12 15 17 10"></polyline>
          <line x1="12" y1="15" x2="12" y2="3"></line>
        </svg>
        Download CITADEL Client (citadel-client.exe)
      </a>
      <div class="file-meta">
        Windows 64-bit • Native Hardware Lockdown • Size: ~350 KB
      </div>
    </div>

    <div class="steps-container">
      <div class="steps-title">Instructions for Candidates:</div>
      <div class="step-item">
        <div class="step-number">1</div>
        <div class="step-text"><strong>Download & Save:</strong> Click the button above to download <code>citadel-client.exe</code> to your laptop.</div>
      </div>
      <div class="step-item">
        <div class="step-number">2</div>
        <div class="step-text"><strong>Launch & Elevate:</strong> Open the file and click <strong>Yes</strong> on the Windows Administrator prompt (UAC) to engage network & keyboard lockdown.</div>
      </div>
      <div class="step-item">
        <div class="step-number">3</div>
        <div class="step-text"><strong>Assessment Launches:</strong> The exam problems and code editor will open automatically in full-screen isolated kiosk mode.</div>
      </div>
    </div>

    <div class="footer-note">
      Server: <span class="tag">172.60.5.98:8443</span> • Proctor Override: <span class="tag">Ctrl+Shift+Alt+F12</span>
    </div>
  </div>
</body>
</html>
"#
}
