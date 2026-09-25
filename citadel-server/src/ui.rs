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

    /* SECURITY BLUR / VISIBILITY ALERT BANNER */
    #blur-warning {
      display: none;
      position: fixed;
      top: 0; left: 0; right: 0;
      background: linear-gradient(90deg, #dc2626, #991b1b);
      color: white;
      font-weight: 700;
      font-size: 13px;
      padding: 10px 16px;
      text-align: center;
      z-index: 10000;
      box-shadow: 0 4px 20px rgba(220, 38, 38, 0.6);
      animation: flash 0.5s infinite alternate;
    }
    @keyframes flash {
      from { opacity: 0.95; }
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
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .logo-badge {
      background: linear-gradient(135deg, #1e3a8a, #0284c7);
      color: #fff;
      font-weight: 800;
      font-size: 14px;
      letter-spacing: 1px;
      padding: 4px 8px;
      border-radius: 6px;
      box-shadow: 0 0 12px rgba(2, 132, 199, 0.4);
    }

    .exam-title {
      font-size: 14px;
      font-weight: 600;
      letter-spacing: -0.2px;
      color: var(--text-main);
    }

    .exam-subtitle {
      font-size: 11px;
      color: var(--text-muted);
    }

    .header-status {
      display: flex;
      align-items: center;
      gap: 20px;
    }

    .status-pill {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: 20px;
      background: rgba(16, 185, 129, 0.1);
      border: 1px solid rgba(16, 185, 129, 0.25);
      font-size: 12px;
      font-weight: 500;
      color: #34d399;
    }

    .status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background-color: var(--accent-green);
      animation: pulse 2s infinite;
    }

    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(0.85); }
    }

    .timer-pill {
      display: flex;
      align-items: center;
      gap: 8px;
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      padding: 4px 12px;
      border-radius: 6px;
      font-family: var(--font-mono);
      font-size: 13px;
      font-weight: 600;
      color: #38bdf8;
    }

    /* MAIN CONTAINER */
    .main-wrapper {
      display: flex;
      flex: 1;
      height: calc(100vh - 56px);
      overflow: hidden;
    }

    /* LEFT PANE - QUESTIONS */
    .left-pane {
      width: 45%;
      border-right: 1px solid var(--border-subtle);
      display: flex;
      flex-direction: column;
      background: var(--bg-surface);
    }

    .tabs-header {
      display: flex;
      background: rgba(6, 9, 15, 0.6);
      border-bottom: 1px solid var(--border-subtle);
      padding: 0 8px;
      gap: 4px;
    }

    .tab-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      padding: 10px 14px;
      font-size: 13px;
      font-weight: 500;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 8px;
      border-bottom: 2px solid transparent;
      transition: all 0.2s;
    }

    .tab-btn:hover {
      color: var(--text-main);
      background: rgba(255, 255, 255, 0.02);
    }

    .tab-btn.active {
      color: var(--accent-cyan);
      border-bottom-color: var(--accent-cyan);
      background: rgba(6, 182, 212, 0.04);
    }

    .badge-points {
      background: #1e293b;
      color: #94a3b8;
      font-size: 11px;
      padding: 2px 6px;
      border-radius: 4px;
      font-family: var(--font-mono);
    }

    .tab-btn.active .badge-points {
      background: rgba(6, 182, 212, 0.2);
      color: var(--accent-cyan);
    }

    .question-content {
      flex: 1;
      overflow-y: auto;
      padding: 24px;
    }

    .q-meta {
      display: flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 14px;
    }

    .badge-diff {
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .diff-easy { background: rgba(16, 185, 129, 0.15); color: #34d399; }
    .diff-medium { background: rgba(245, 158, 11, 0.15); color: #fbbf24; }
    .diff-hard { background: rgba(239, 68, 68, 0.15); color: #f87171; }

    .tag-item {
      font-size: 11px;
      color: var(--text-dim);
      background: rgba(255, 255, 255, 0.03);
      padding: 2px 6px;
      border-radius: 4px;
      border: 1px solid var(--border-subtle);
    }

    h2.q-title {
      font-size: 18px;
      font-weight: 700;
      margin-bottom: 16px;
      letter-spacing: -0.3px;
    }

    .section-title {
      font-size: 12px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: var(--text-muted);
      margin-top: 20px;
      margin-bottom: 8px;
    }

    .q-desc {
      font-size: 14px;
      line-height: 1.6;
      color: #cbd5e1;
      margin-bottom: 16px;
      white-space: pre-line;
    }

    .code-block {
      background: #090d16;
      border: 1px solid var(--border-subtle);
      border-radius: 6px;
      padding: 10px 14px;
      font-family: var(--font-mono);
      font-size: 13px;
      color: #e2e8f0;
      white-space: pre;
      overflow-x: auto;
      margin-bottom: 12px;
    }

    .test-case-box {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 12px;
      margin-bottom: 12px;
    }

    /* RIGHT PANE - EDITOR & CONSOLE */
    .right-pane {
      width: 55%;
      display: flex;
      flex-direction: column;
      background: var(--bg-editor);
    }

    .editor-toolbar {
      height: 44px;
      background: var(--bg-surface);
      border-bottom: 1px solid var(--border-subtle);
      padding: 0 16px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .toolbar-left {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .lang-select {
      background: var(--bg-surface-elevated);
      color: var(--text-main);
      border: 1px solid var(--border-subtle);
      border-radius: 6px;
      padding: 5px 10px;
      font-size: 12px;
      font-weight: 500;
      outline: none;
      cursor: pointer;
    }

    .lang-select:focus { border-color: var(--border-focus); }

    .editor-container {
      flex: 1;
      position: relative;
    }

    #code-editor {
      width: 100%;
      height: 100%;
      background: transparent;
      color: #e2e8f0;
      border: none;
      outline: none;
      resize: none;
      padding: 16px;
      font-family: var(--font-mono);
      font-size: 14px;
      line-height: 1.6;
      tab-size: 4;
      user-select: text;
    }

    /* BOTTOM CONTROLS & TEST RESULTS CONSOLE */
    .console-pane {
      height: 220px;
      background: #05070d;
      border-top: 1px solid var(--border-subtle);
      display: flex;
      flex-direction: column;
    }

    .console-header {
      height: 36px;
      background: var(--bg-surface);
      border-bottom: 1px solid var(--border-subtle);
      padding: 0 16px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .console-title {
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .action-buttons {
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .btn {
      padding: 6px 14px;
      border-radius: 6px;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 6px;
      transition: all 0.2s;
      border: none;
    }

    .btn-secondary {
      background: #1e293b;
      color: #cbd5e1;
      border: 1px solid var(--border-subtle);
    }

    .btn-secondary:hover {
      background: #334155;
      color: #fff;
    }

    .btn-primary {
      background: linear-gradient(135deg, #2563eb, #1d4ed8);
      color: #fff;
      box-shadow: 0 2px 8px rgba(37, 99, 235, 0.3);
    }

    .btn-primary:hover {
      background: linear-gradient(135deg, #3b82f6, #2563eb);
      box-shadow: 0 4px 12px rgba(37, 99, 235, 0.5);
    }

    .console-body {
      flex: 1;
      padding: 12px 16px;
      overflow-y: auto;
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.5;
    }

    .res-badge {
      display: inline-block;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 12px;
      font-weight: 600;
      margin-bottom: 8px;
    }
    .res-accepted { background: rgba(16, 185, 129, 0.2); color: #34d399; }
    .res-failed { background: rgba(239, 68, 68, 0.2); color: #f87171; }
  </style>
</head>
<body>
  <!-- DYNAMIC SECURITY WARNING BANNER -->
  <div id="security-toast">⚠️ <span id="security-toast-msg">Security Violation</span></div>
  <div id="blur-warning">
    ⚠️ CRITICAL INTEGRITY VIOLATION: Assessment window was minimized or switched away! Incident recorded in proctor audit log.
  </div>

  <header>
    <div class="brand">
      <div class="logo-badge">CITADEL</div>
      <div>
        <div class="exam-title">CAMPUS PLACEMENT ASSESSMENT 2026</div>
        <div class="exam-subtitle">Hardware Isolated • Zero Internet • WFP Filter Active</div>
      </div>
    </div>
    <div class="header-status">
      <div class="status-pill">
        <span class="status-dot"></span>
        <span id="conn-status">LAN Air-Gapped (172.60.5.98)</span>
      </div>
      <div class="timer-pill">
        <span>⏱️</span>
        <span id="exam-timer">01:30:00</span>
      </div>
    </div>
  </header>

  <div class="main-wrapper">
    <!-- LEFT PANE: QUESTIONS -->
    <div class="left-pane">
      <div class="tabs-header" id="question-tabs">
        <!-- Injected via JS -->
      </div>
      <div class="question-content" id="question-body">
        <!-- Injected via JS -->
      </div>
    </div>

    <!-- RIGHT PANE: CODE EDITOR & SUBMISSIONS -->
    <div class="right-pane">
      <div class="editor-toolbar">
        <div class="toolbar-left">
          <select class="lang-select" id="lang-selector" onchange="changeLanguage()">
            <option value="python">Python 3.11</option>
            <option value="cpp">C++ (GCC 13 / C++20)</option>
            <option value="java">Java 17 (OpenJDK)</option>
          </select>
          <span style="font-size: 12px; color: var(--text-dim);">UTF-8 • Tab: 4 spaces</span>
        </div>
        <div class="action-buttons">
          <button class="btn btn-secondary" onclick="runCode(true)">
            ▶ Run Sample Tests
          </button>
          <button class="btn btn-primary" onclick="runCode(false)">
            🚀 Submit Final Code
          </button>
        </div>
      </div>

      <div class="editor-container">
        <textarea id="code-editor" spellcheck="false" placeholder="Write your solution here..."></textarea>
      </div>

      <div class="console-pane">
        <div class="console-header">
          <div class="console-title">Execution Console & Validation Output</div>
          <span id="runtime-meta" style="font-size: 11px; color: var(--text-dim); font-family: var(--font-mono);"></span>
        </div>
        <div class="console-body" id="results-console">
          <span style="color: var(--text-dim);">Click "Run Sample Tests" to execute against public cases, or "Submit Final Code" for hidden evaluation.</span>
        </div>
      </div>
    </div>
  </div>

  <script>
    const CANDIDATE_ID = "CAND-" + Math.floor(100000 + Math.random() * 900000);
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
      showSecurityToast('Right-click context menu is disabled.');
      return false;
    });

    // 2. Disable Drag-and-Drop
    document.addEventListener('dragstart', e => e.preventDefault());
    document.addEventListener('drop', e => e.preventDefault());

    function showSecurityToast(msg) {
      const toast = document.getElementById('security-toast');
      const text = document.getElementById('security-toast-msg');
      if (toast && text) {
        text.innerText = msg;
        toast.style.display = 'block';
        clearTimeout(window._toastTimer);
        window._toastTimer = setTimeout(() => {
          toast.style.display = 'none';
        }, 3500);
      }
    }

    // 3. Disable Copying Exam Content to External Clipboard
    document.addEventListener('copy', e => {
      if (document.activeElement && document.activeElement.id === 'code-editor') {
        // Allowed inside editor for candidate's own code
      } else {
        e.preventDefault();
        showSecurityToast('Copying exam questions or instructions is strictly prohibited.');
        reportSecurityViolation('CLIPBOARD_COPY_ATTEMPT', 'Attempted to copy exam question content');
      }
    });

    // 4. Disable Pasting External Code
    document.addEventListener('paste', e => {
      e.preventDefault();
      showSecurityToast('Pasting external content is blocked. Please type code directly.');
      reportSecurityViolation('CLIPBOARD_PASTE_ATTEMPT', 'Attempted to paste external clipboard content');
    });

    // 5. Disable DevTools Shortcuts (F12, Ctrl+Shift+I, Ctrl+Shift+J, Ctrl+U)
    document.addEventListener('keydown', e => {
      if (
        e.key === 'F12' ||
        (e.ctrlKey && e.shiftKey && (e.key === 'I' || e.key === 'i' || e.key === 'J' || e.key === 'j' || e.key === 'C' || e.key === 'c')) ||
        (e.ctrlKey && (e.key === 'u' || e.key === 'U' || e.key === 'p' || e.key === 'P' || e.key === 's' || e.key === 'S'))
      ) {
        e.preventDefault();
        return false;
      }
    });

    // 6. Monitor Window Blur & Visibility Change (Focus-Loss / Window Switching)
    window.addEventListener('blur', () => {
      const banner = document.getElementById('blur-warning');
      if (banner) banner.style.display = 'block';
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText('').catch(() => {});
      }
      reportSecurityViolation('WINDOW_BLUR', 'Candidate assessment window lost focus');
    });

    window.addEventListener('focus', () => {
      setTimeout(() => {
        const banner = document.getElementById('blur-warning');
        if (banner) banner.style.display = 'none';
      }, 3000);
    });

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        const banner = document.getElementById('blur-warning');
        if (banner) banner.style.display = 'block';
        reportSecurityViolation('TAB_SWITCHED_OR_MINIMIZED', 'Assessment tab minimized or backgrounded');
      }
    });

    // Report security violation to server audit log
    async function reportSecurityViolation(eventType, details) {
      try {
        await fetch('/api/v1/integrity/event', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            candidate_id: CANDIDATE_ID,
            event_type: eventType,
            details: details,
            severity: 'HIGH',
          }),
        });
      } catch (err) {}
    }

    // Proctor Heartbeat loop (every 15s)
    setInterval(async () => {
      try {
        await fetch('/api/v1/integrity/heartbeat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            candidate_id: CANDIDATE_ID,
            active_question: currentQIndex + 1,
            is_window_focused: document.hasFocus() && !document.hidden,
          }),
        });
      } catch (err) {}
    }, 15000);

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
        document.getElementById('results-console').innerHTML = `<span style="color: #ef4444;">Error connecting to CITADEL Exam Server: ${err.message}</span>`;
      }
    }

    function renderTabs() {
      const tabsHeader = document.getElementById('question-tabs');
      tabsHeader.innerHTML = questions.map((q, idx) => `
        <button class="tab-btn ${idx === 0 ? 'active' : ''}" onclick="loadQuestion(${idx})">
          <span>Q${q.number}</span>
          <span class="badge-points">${q.points} pts</span>
        </button>
      `).join('');
    }

    function loadQuestion(idx) {
      // Save current code
      if (questions[currentQIndex]) {
        codeStorage[`${questions[currentQIndex].id}_${currentLang}`] = document.getElementById('code-editor').value;
      }

      currentQIndex = idx;
      const q = questions[idx];

      // Update active tab style
      const tabBtns = document.querySelectorAll('.tab-btn');
      tabBtns.forEach((btn, i) => {
        btn.classList.toggle('active', i === idx);
      });

      // Render question content
      const diffClass = q.difficulty === 'Easy' ? 'diff-easy' : (q.difficulty === 'Medium' ? 'diff-medium' : 'diff-hard');
      const tagsHtml = q.tags.map(t => `<span class="tag-item">${t}</span>`).join('');

      let samplesHtml = '';
      if (q.sample_cases && q.sample_cases.length > 0) {
        samplesHtml = '<div class="section-title">Sample Cases</div>' + q.sample_cases.map((sc, i) => `
          <div class="test-case-box">
            <div style="font-size: 11px; font-weight: 600; color: #94a3b8; margin-bottom: 6px;">Sample Case ${i + 1}</div>
            <div style="font-size: 12px; color: #94a3b8;">Input:</div>
            <div class="code-block">${sc.input}</div>
            <div style="font-size: 12px; color: #94a3b8;">Expected Output:</div>
            <div class="code-block">${sc.expected_output}</div>
            ${sc.explanation ? `<div style="font-size: 12px; color: #64748b; font-style: italic;">${sc.explanation}</div>` : ''}
          </div>
        `).join('');
      }

      document.getElementById('question-body').innerHTML = `
        <div class="q-meta">
          <span class="badge-diff ${diffClass}">${q.difficulty}</span>
          <span class="badge-points">${q.points} Points</span>
          ${tagsHtml}
        </div>
        <h2 class="q-title">${q.number}. ${q.title}</h2>
        <div class="q-desc">${q.description}</div>

        <div class="section-title">Input Format</div>
        <div class="q-desc">${q.input_format}</div>

        <div class="section-title">Output Format</div>
        <div class="q-desc">${q.output_format}</div>

        <div class="section-title">Constraints</div>
        <ul style="padding-left: 20px; font-size: 13px; color: #cbd5e1; margin-bottom: 16px;">
          ${q.constraints.map(c => `<li style="margin-bottom: 4px;"><code>${c}</code></li>`).join('')}
        </ul>

        ${samplesHtml}
      `;

      // Restore code
      const storageKey = `${q.id}_${currentLang}`;
      if (codeStorage[storageKey]) {
        document.getElementById('code-editor').value = codeStorage[storageKey];
      } else if (q.starter_templates && q.starter_templates[currentLang]) {
        document.getElementById('code-editor').value = q.starter_templates[currentLang];
      } else {
        document.getElementById('code-editor').value = '';
      }
    }

    function changeLanguage() {
      currentLang = document.getElementById('lang-selector').value;
      const q = questions[currentQIndex];
      const storageKey = `${q.id}_${currentLang}`;
      if (codeStorage[storageKey]) {
        document.getElementById('code-editor').value = codeStorage[storageKey];
      } else if (q.starter_templates && q.starter_templates[currentLang]) {
        document.getElementById('code-editor').value = q.starter_templates[currentLang];
      } else {
        document.getElementById('code-editor').value = '';
      }
    }

    async function runCode(isSample) {
      const q = questions[currentQIndex];
      const code = document.getElementById('code-editor').value;
      const consoleEl = document.getElementById('results-console');
      const metaEl = document.getElementById('runtime-meta');

      consoleEl.innerHTML = `<span style="color: #38bdf8;">⏳ Compiling & executing solution on server...</span>`;
      metaEl.innerText = "";

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: q.id,
            language: currentLang,
            source_code: code,
            is_sample_run: isSample,
            candidate_id: CANDIDATE_ID,
          }),
        });

        const data = await res.json();
        const isOk = data.status === 'Accepted';
        const badgeClass = isOk ? 'res-accepted' : 'res-failed';

        metaEl.innerText = `Runtime: ${data.runtime_ms} ms • Memory: ${data.memory_mb} MB`;
        consoleEl.innerHTML = `
          <div>
            <span class="res-badge ${badgeClass}">${data.status}</span>
            <span style="font-weight: 600; color: #fff; margin-left: 8px;">
              Score: ${data.score}/${q.points} pts (Passed ${data.passed_cases}/${data.total_cases} Cases)
            </span>
          </div>
          <div style="margin-top: 8px; color: ${isOk ? '#34d399' : '#f87171'};">
            ${data.details}
          </div>
        `;
      } catch (err) {
        consoleEl.innerHTML = `<span style="color: #ef4444;">Execution failed: ${err.message}</span>`;
      }
    }

    function startTimer() {
      let secondsLeft = 90 * 60;
      setInterval(() => {
        if (secondsLeft <= 0) return;
        secondsLeft--;
        const h = String(Math.floor(secondsLeft / 3600)).padStart(2, '0');
        const m = String(Math.floor((secondsLeft % 3600) / 60)).padStart(2, '0');
        const s = String(secondsLeft % 60).padStart(2, '0');
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

pub fn render_proctor_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>CITADEL — Proctor & Recruiter Live Command Center</title>
  <style>
    :root {
      --bg-base: #030712;
      --bg-surface: #0f172a;
      --bg-surface-elevated: #1e293b;
      --border-subtle: rgba(255, 255, 255, 0.08);
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      --text-dim: #64748b;
      --accent-blue: #3b82f6;
      --accent-cyan: #06b6d4;
      --accent-green: #10b981;
      --accent-red: #ef4444;
      --accent-amber: #f59e0b;
      --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
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
    }

    header {
      background: var(--bg-surface);
      border-bottom: 1px solid var(--border-subtle);
      padding: 14px 28px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .brand-title {
      font-size: 16px;
      font-weight: 700;
      letter-spacing: -0.3px;
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .badge-live {
      background: rgba(16, 185, 129, 0.15);
      border: 1px solid rgba(16, 185, 129, 0.3);
      color: #34d399;
      font-size: 11px;
      font-weight: 600;
      padding: 2px 8px;
      border-radius: 12px;
      display: flex;
      align-items: center;
      gap: 6px;
    }

    .pulse-dot {
      width: 6px;
      height: 6px;
      background: #10b981;
      border-radius: 50%;
      animation: pulse 1.5s infinite;
    }
    @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

    .container {
      max-width: 1300px;
      width: 100%;
      margin: 0 auto;
      padding: 28px;
      flex: 1;
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      gap: 20px;
      margin-bottom: 28px;
    }

    .stat-card {
      background: var(--bg-surface);
      border: 1px solid var(--border-subtle);
      border-radius: 12px;
      padding: 20px;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    }

    .stat-label {
      font-size: 12px;
      color: var(--text-muted);
      text-transform: uppercase;
      font-weight: 600;
      letter-spacing: 0.5px;
      margin-bottom: 8px;
    }

    .stat-value {
      font-size: 28px;
      font-weight: 700;
      font-family: var(--font-mono);
    }

    .card-tabs {
      background: var(--bg-surface);
      border: 1px solid var(--border-subtle);
      border-radius: 12px;
      overflow: hidden;
    }

    .tab-bar {
      display: flex;
      background: rgba(0, 0, 0, 0.2);
      border-bottom: 1px solid var(--border-subtle);
      padding: 0 16px;
    }

    .t-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      padding: 14px 18px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      border-bottom: 2px solid transparent;
      transition: all 0.2s;
    }

    .t-btn.active {
      color: var(--accent-cyan);
      border-bottom-color: var(--accent-cyan);
    }

    .tab-view {
      padding: 20px;
      display: none;
    }
    .tab-view.active { display: block; }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
    }

    th {
      text-align: left;
      padding: 12px 14px;
      color: var(--text-muted);
      font-weight: 600;
      border-bottom: 1px solid var(--border-subtle);
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    td {
      padding: 12px 14px;
      border-bottom: 1px solid var(--border-subtle);
      color: #cbd5e1;
    }

    tr:hover {
      background: rgba(255, 255, 255, 0.02);
    }

    .status-badge {
      display: inline-block;
      padding: 3px 8px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      text-transform: uppercase;
    }
    .badge-active { background: rgba(16, 185, 129, 0.15); color: #34d399; }
    .badge-flagged { background: rgba(239, 68, 68, 0.2); color: #f87171; }
    .badge-disqualified { background: rgba(100, 116, 139, 0.2); color: #94a3b8; }

    .btn-disqualify {
      background: rgba(239, 68, 68, 0.15);
      border: 1px solid rgba(239, 68, 68, 0.3);
      color: #f87171;
      padding: 4px 10px;
      border-radius: 6px;
      font-size: 11px;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-disqualify:hover {
      background: #ef4444;
      color: white;
    }
  </style>
</head>
<body>
  <header>
    <div class="brand-title">
      🛡️ CITADEL FLEET PROCTOR & RECRUITER COMMAND
      <span class="badge-live"><span class="pulse-dot"></span> LIVE FLEET ONLINE</span>
    </div>
    <div style="font-size: 12px; color: var(--text-muted); font-family: var(--font-mono);">
      Server: 172.60.5.98:8443 • Zero-Internet Campus Subnet
    </div>
  </header>

  <div class="container">
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">Total Connected Candidates</div>
        <div class="stat-value" id="stat-total-cands" style="color: #38bdf8;">--</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Flagged Security Incidents</div>
        <div class="stat-value" id="stat-flagged" style="color: #f87171;">--</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Total Code Submissions</div>
        <div class="stat-value" id="stat-subs" style="color: #34d399;">--</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Security Fleet Status</div>
        <div class="stat-value" style="color: #a855f7; font-size: 20px; line-height: 28px;">AIR-GAPPED 100%</div>
      </div>
    </div>

    <div class="card-tabs">
      <div class="tab-bar">
        <button class="t-btn active" onclick="switchTab('tab-cands', this)">Candidate Fleet Monitor</button>
        <button class="t-btn" onclick="switchTab('tab-threats', this)">Live Security Threat Stream</button>
        <button class="t-btn" onclick="switchTab('tab-subs', this)">Submission Score Records</button>
      </div>

      <div class="tab-view active" id="tab-cands">
        <table>
          <thead>
            <tr>
              <th>Candidate ID</th>
              <th>IP Address</th>
              <th>Active Question</th>
              <th>Score</th>
              <th>Violations</th>
              <th>Status</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody id="cands-tbody">
            <tr><td colspan="7" style="text-align: center; color: var(--text-dim);">Loading fleet candidates...</td></tr>
          </tbody>
        </table>
      </div>

      <div class="tab-view" id="tab-threats">
        <table>
          <thead>
            <tr>
              <th>Timestamp</th>
              <th>Candidate ID</th>
              <th>Threat Type</th>
              <th>Incident Details</th>
              <th>Severity</th>
            </tr>
          </thead>
          <tbody id="threats-tbody">
            <tr><td colspan="5" style="text-align: center; color: var(--text-dim);">No security violations logged.</td></tr>
          </tbody>
        </table>
      </div>

      <div class="tab-view" id="tab-subs">
        <table>
          <thead>
            <tr>
              <th>Submission ID</th>
              <th>Candidate ID</th>
              <th>Question</th>
              <th>Language</th>
              <th>Test Cases Passed</th>
              <th>Score</th>
              <th>Status</th>
              <th>Timestamp</th>
            </tr>
          </thead>
          <tbody id="subs-tbody">
            <tr><td colspan="8" style="text-align: center; color: var(--text-dim);">No submissions recorded yet.</td></tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>

  <script>
    function switchTab(tabId, btn) {
      document.querySelectorAll('.t-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.tab-view').forEach(v => v.classList.remove('active'));
      btn.classList.add('active');
      document.getElementById(tabId).classList.add('active');
    }

    async function fetchMetrics() {
      try {
        const res = await fetch('/api/v1/proctor/metrics');
        const data = await res.json();

        document.getElementById('stat-total-cands').innerText = data.total_candidates;
        document.getElementById('stat-flagged').innerText = data.flagged_candidates;
        document.getElementById('stat-subs').innerText = data.total_submissions;

        // Render Candidates
        const candsBody = document.getElementById('cands-tbody');
        if (data.candidates.length > 0) {
          candsBody.innerHTML = data.candidates.map(c => `
            <tr>
              <td style="font-family: var(--font-mono); font-weight: 600;">${c.candidate_id}</td>
              <td><code>${c.ip_address}</code></td>
              <td>Question ${c.active_question}</td>
              <td style="font-weight: 700; color: #38bdf8;">${c.total_score} pts</td>
              <td style="color: ${c.violations_count > 0 ? '#f87171' : '#34d399'}; font-weight: 600;">${c.violations_count}</td>
              <td>
                <span class="status-badge ${c.status === 'Active' ? 'badge-active' : (c.status === 'Flagged' ? 'badge-flagged' : 'badge-disqualified')}">
                  ${c.status}
                </span>
              </td>
              <td>
                ${c.status !== 'Disqualified' ? `<button class="btn-disqualify" onclick="disqualify('${c.candidate_id}')">Disqualify</button>` : '<span style="color: #64748b;">Locked</span>'}
              </td>
            </tr>
          `).join('');
        }

        // Render Threat Incidents
        const threatsBody = document.getElementById('threats-tbody');
        if (data.recent_violations.length > 0) {
          threatsBody.innerHTML = data.recent_violations.map(v => `
            <tr>
              <td style="font-family: var(--font-mono); font-size: 11px;">${v.timestamp.split('T')[1].split('.')[0]}</td>
              <td style="font-weight: 600;">${v.candidate_id}</td>
              <td style="color: #f87171; font-weight: 600;">${v.event_type}</td>
              <td>${v.details}</td>
              <td><span class="status-badge badge-flagged">${v.severity}</span></td>
            </tr>
          `).join('');
        }

        // Render Submissions
        const subsBody = document.getElementById('subs-tbody');
        if (data.recent_submissions.length > 0) {
          subsBody.innerHTML = data.recent_submissions.map(s => `
            <tr>
              <td style="font-family: var(--font-mono); font-size: 11px;">${s.submission_id}</td>
              <td style="font-weight: 600;">${s.candidate_id}</td>
              <td>${s.question_id}</td>
              <td><code>${s.language}</code></td>
              <td>${s.passed_cases} / ${s.total_cases}</td>
              <td style="color: #38bdf8; font-weight: 600;">${s.score}</td>
              <td><span class="status-badge badge-active">${s.status}</span></td>
              <td style="font-size: 11px; color: var(--text-dim);">${s.timestamp.split('T')[1].split('.')[0]}</td>
            </tr>
          `).join('');
        }

      } catch (err) {}
    }

    async function disqualify(candId) {
      if (confirm(`Are you sure you want to disqualify candidate ${candId}?`)) {
        await fetch(`/api/v1/proctor/candidates/${candId}/disqualify`, { method: 'POST' });
        fetchMetrics();
      }
    }

    fetchMetrics();
    setInterval(fetchMetrics, 2000);
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
        Windows 64-bit • Embedded UAC Manifest • Size: ~350 KB
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
