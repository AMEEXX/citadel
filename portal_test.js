
    // =========================================================================
    // STARTER TEMPLATES & STATE
    // =========================================================================
    const CANDIDATE_ID = "CAND-" + Math.floor(1000 + Math.random() * 9000);
    let currentLang = "python";
    let samplePassed = false;
    let cachedQuestion = null;
    let latestDiffs = [];
    let activeCaseIdx = 0;
    let editor = null;

    // Per-language syntax color palette (Cold-blue/pure black harmonious accents)
    const LANG_CONFIG = {
      python: {
        ext: '.py',
        file: 'main.py',
        label: 'PYTHON',
        accent: '#c07a5a',
        mode: 'ace/mode/python',
        palette: {
          keyword: '#c07a5a',
          string: '#7aaa60',
          comment: '#526058',
          number: '#6aaa7a',
          function: '#c8a858',
          type: '#6a9ab8',
          operator: '#506058',
          punctuation: '#303838',
          variable: '#b0bccf',
        }
      },
      cpp: {
        ext: '.cpp',
        file: 'main.cpp',
        label: 'C++ 17',
        accent: '#5b8fc7',
        mode: 'ace/mode/c_cpp',
        palette: {
          keyword: '#5b8fc7',
          string: '#b08060',
          comment: '#505c6c',
          number: '#6a9e6a',
          function: '#c8b86a',
          type: '#6aaac0',
          operator: '#50607a',
          punctuation: '#303848',
          variable: '#b0bccf',
        }
      },
      java: {
        ext: '.java',
        file: 'Solution.java',
        label: 'JAVA 17',
        accent: '#8878b8',
        mode: 'ace/mode/java',
        palette: {
          keyword: '#8878b8',
          string: '#88aa68',
          comment: '#5c5868',
          number: '#78b080',
          function: '#78a8c0',
          type: '#98a870',
          operator: '#506068',
          punctuation: '#303040',
          variable: '#b0bccf',
        }
      }
    };

    const STARTERS = {
      python: `import sys

def two_sum(nums, target):
    # Write your solution here
    # Return a list containing the two indices [i, j]
    seen = {}
    for i, n in enumerate(nums):
        diff = target - n
        if diff in seen:
            return [seen[diff], i]
        seen[n] = i
    return []

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        target = int(tokens[-1])
        nums = [int(x) for x in tokens[:-1]]
        ans = two_sum(nums, target)
        if ans and len(ans) == 2:
            print(f"{min(ans[0], ans[1])} {max(ans[0], ans[1])}")
`,
      cpp: `#include <iostream>
#include <vector>
#include <unordered_map>
#include <algorithm>

using namespace std;

vector<int> twoSum(vector<int>& nums, int target) {
    // Write your solution here
    // Return vector containing the two indices
    unordered_map<int, int> seen;
    for (int i = 0; i < (int)nums.size(); ++i) {
        int diff = target - nums[i];
        if (seen.count(diff)) {
            return {seen[diff], i};
        }
        seen[nums[i]] = i;
    }
    return {};
}

int main() {
    ios_base::sync_with_stdio(false);
    cin.tie(NULL);

    vector<int> nums;
    int val;
    while (cin >> val) {
        nums.push_back(val);
    }
    if (nums.size() >= 2) {
        int target = nums.back();
        nums.pop_back();
        vector<int> ans = twoSum(nums, target);
        if (ans.size() == 2) {
            cout << min(ans[0], ans[1]) << " " << max(ans[0], ans[1]) << "\\n";
        }
    }
    return 0;
}
`,
      java: `import java.util.*;

public class Solution {
    public static int[] twoSum(int[] nums, int target) {
        // Write your solution here
        // Return int array with two indices
        Map<Integer, Integer> seen = new HashMap<>();
        for (int i = 0; i < nums.length; i++) {
            int diff = target - nums[i];
            if (seen.containsKey(diff)) {
                return new int[]{seen.get(diff), i};
            }
            seen.put(nums[i], i);
        }
        return new int[]{};
    }

    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        List<Integer> list = new ArrayList<>();
        while (sc.hasNextInt()) {
            list.add(sc.nextInt());
        }
        if (list.size() >= 2) {
            int target = list.remove(list.size() - 1);
            int[] nums = new int[list.size()];
            for (int i = 0; i < list.size(); i++) {
                nums[i] = list.get(i);
            }
            int[] ans = twoSum(nums, target);
            if (ans != null && ans.length == 2) {
                System.out.println(Math.min(ans[0], ans[1]) + " " + Math.max(ans[0], ans[1]));
            }
        }
    }
}
`
    };

    const codeStore = { ...STARTERS };

    // =========================================================================
    // ACE CODE EDITOR INITIALIZATION & PALETTE SYNC
    // =========================================================================
    function applyLanguagePalette(langKey) {
      const cfg = LANG_CONFIG[langKey];
      if (!cfg) return;

      const root = document.documentElement;
      root.style.setProperty('--lang-accent', cfg.accent);
      root.style.setProperty('--tok-keyword', cfg.palette.keyword);
      root.style.setProperty('--tok-string', cfg.palette.string);
      root.style.setProperty('--tok-comment', cfg.palette.comment);
      root.style.setProperty('--tok-number', cfg.palette.number);
      root.style.setProperty('--tok-function', cfg.palette.function);
      root.style.setProperty('--tok-type', cfg.palette.type);
      root.style.setProperty('--tok-operator', cfg.palette.operator);
      root.style.setProperty('--tok-punctuation', cfg.palette.punctuation);
      root.style.setProperty('--tok-variable', cfg.palette.variable);

      document.getElementById('filename-display').innerText = cfg.file;
      document.getElementById('status-lang-text').innerText = cfg.label;

      document.querySelectorAll('.lang-tab').forEach(el => el.classList.remove('active'));
      const activeTab = document.getElementById('tab-' + langKey);
      if (activeTab) activeTab.classList.add('active');
    }

    function initAceEditor() {
      editor = ace.edit("code-editor");
      editor.setTheme("ace/theme/one_dark");
      editor.setFontSize("13.5px");
      editor.setShowPrintMargin(false);

      editor.setOptions({
        tabSize: 4,
        useSoftTabs: true,
        showGutter: true,
        highlightActiveLine: true,
        wrap: true,
        behavioursEnabled: true,
        wrapBehavioursEnabled: true,
        autoScrollEditorIntoView: true
      });

      applyLanguagePalette('python');
      editor.session.setMode("ace/mode/python");
      editor.setValue(STARTERS.python, -1);

      // Cursor position updates
      editor.selection.on('changeCursor', () => {
        const pos = editor.getCursorPosition();
        document.getElementById('status-pos').innerText = `Ln ${pos.row + 1}, Col ${pos.column + 1}`;
        const totalLines = editor.session.getLength();
        const progress = Math.min(100, Math.max(10, Math.round(((pos.row + 1) / totalLines) * 100)));
        document.getElementById('status-scroll-bar').style.width = progress + '%';
      });

      editor.session.on('change', () => {
        if (samplePassed) {
          samplePassed = false;
          lockSubmitButton("Code Modified — Re-run Sample Cases");
        }
      });
    }

    function switchLanguage(targetLang) {
      if (!editor || targetLang === currentLang) return;
      codeStore[currentLang] = editor.getValue();
      currentLang = targetLang;

      const cfg = LANG_CONFIG[targetLang];
      applyLanguagePalette(targetLang);

      editor.session.setMode(cfg.mode);
      editor.setValue(codeStore[targetLang] || STARTERS[targetLang], -1);

      samplePassed = false;
      lockSubmitButton("Run Sample Cases First");
    }

    function resetStarterTemplate() {
      if (!editor) return;
      if (confirm("Reset current editor code to starter template? Any unsaved edits will be lost.")) {
        editor.setValue(STARTERS[currentLang], -1);
        samplePassed = false;
        lockSubmitButton("Run Sample Cases First");
      }
    }

    // =========================================================================
    // SUBMISSION & JUDGE EVALUATION (RUN CODE & SUBMIT)
    // =========================================================================
    function lockSubmitButton(label) {
      const btn = document.getElementById('submit-btn');
      btn.disabled = true;
      btn.className = "btn-action btn-submit locked";
      document.getElementById('lock-icon').innerText = "🔒";
      document.getElementById('submit-label').innerText = label;
    }

    function unlockSubmitButton() {
      const btn = document.getElementById('submit-btn');
      btn.disabled = false;
      btn.className = "btn-action btn-submit unlocked";
      document.getElementById('lock-icon').innerText = "✓";
      document.getElementById('submit-label').innerText = "Submit Solution";
    }

    function escapeHtml(text) {
      if (!text) return "";
      return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
    }

    async function runSampleCases() {
      if (!editor) return;
      const consoleView = document.getElementById('view-console');
      switchDrawerTab('tests');
      document.getElementById('case-actual').innerHTML = '<span style="color: #93c5fd;">⏳ Compiling and executing on Citadel Judge Sandbox...</span>';

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: 'q1-two-sum',
            language: currentLang,
            source_code: editor.getValue(),
            is_sample_run: true,
            candidate_id: CANDIDATE_ID,
          }),
        });

        const data = await res.json();
        latestDiffs = data.sample_diffs || [];

        updateCasePills();
        renderCaseDetails(activeCaseIdx);

        if (data.status === 'Accepted') {
          samplePassed = true;
          unlockSubmitButton();
          consoleView.innerHTML = `<span style="color: #34d399; font-weight: 700;">✓ ACCEPTED</span> — All ${data.passed_cases}/${data.total_cases} sample test cases passed (${data.runtime_ms} ms).\\n\\n` +
            `Submit button is now <strong style="color: #34d399;">UNLOCKED</strong>! Click 'Submit Solution' to grade against hidden test cases.`;
        } else {
          samplePassed = false;
          lockSubmitButton("Sample Cases Failed");
          consoleView.innerHTML = `<span style="color: #f87171; font-weight: 700;">✗ ${data.status.toUpperCase()}</span> (${data.passed_cases}/${data.total_cases} Sample Cases Passed)\\n\\n${data.details}`;
        }
      } catch (err) {
        document.getElementById('case-actual').innerText = "Execution failed: " + err.message;
        consoleView.innerText = "Error: " + err.message;
      }
    }

    async function submitFinalCode() {
      if (!editor) return;
      if (!samplePassed) {
        alert("Please run and pass all sample test cases before submitting.");
        return;
      }

      switchDrawerTab('console');
      const consoleView = document.getElementById('view-console');
      consoleView.innerHTML = '<span style="color: #93c5fd;">⏳ Evaluating final submission against all hidden test cases in sandbox...</span>';

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: 'q1-two-sum',
            language: currentLang,
            source_code: editor.getValue(),
            is_sample_run: false,
            candidate_id: CANDIDATE_ID,
          }),
        });

        const data = await res.json();
        if (data.status === 'Accepted') {
          consoleView.innerHTML = `<span style="color: #34d399; font-weight: 700; font-size: 14px;">🎉 CONGRATULATIONS! ALL TEST CASES PASSED!</span>\\n\\n` +
            `• Status: ACCEPTED\\n` +
            `• Score Awarded: ${data.score}/100 Points\\n` +
            `• Passed Test Cases: ${data.passed_cases}/${data.total_cases} (Sample + Hidden)\\n` +
            `• Execution Time: ${data.runtime_ms} ms\\n\\n` +
            `Your submission has been recorded by the exam proctor server.`;
        } else {
          consoleView.innerHTML = `<span style="color: #f87171; font-weight: 700; font-size: 14px;">✗ ${data.status.toUpperCase()}</span>\\n\\n` +
            `• Passed: ${data.passed_cases}/${data.total_cases} Test Cases\\n` +
            `• Score: ${data.score}/100 Points\\n` +
            `• Details: ${data.details}\\n\\n` +
            `Review your logic, corner cases (negative numbers, duplicates, large bounds) and resubmit.`;
        }
      } catch (err) {
        consoleView.innerHTML = `<span style="color: #ef4444;">Final submission error: ${err.message}</span>`;
      }
    }

    function updateCasePills() {
      const container = document.getElementById('case-pill-container');
      const cases = cachedQuestion ? cachedQuestion.sample_cases : [
        {input: "2 7 11 15 9", expected_output: "0 1"},
        {input: "3 2 4 6", expected_output: "1 2"},
        {input: "3 3 6", expected_output: "0 1"}
      ];
      container.innerHTML = cases.map((tc, idx) => {
        let badgeClass = '';
        if (latestDiffs[idx]) {
          badgeClass = latestDiffs[idx].is_passed ? 'pass' : 'fail';
        }
        return `<button class="case-btn ${idx === activeCaseIdx ? 'active' : ''} ${badgeClass}" onclick="selectCaseView(${idx})">Case ${idx + 1}</button>`;
      }).join('');
    }

    function selectCaseView(idx) {
      activeCaseIdx = idx;
      updateCasePills();
      renderCaseDetails(idx);
    }

    function renderCaseDetails(idx) {
      const cases = cachedQuestion ? cachedQuestion.sample_cases : [
        {input: "2 7 11 15 9", expected_output: "0 1"},
        {input: "3 2 4 6", expected_output: "1 2"},
        {input: "3 3 6", expected_output: "0 1"}
      ];
      const tc = cases[idx] || cases[0];
      document.getElementById('case-input').innerText = tc.input;
      document.getElementById('case-expected').innerText = tc.expected_output;

      const actualBox = document.getElementById('case-actual');
      if (latestDiffs[idx]) {
        const diff = latestDiffs[idx];
        if (diff.is_passed) {
          actualBox.innerHTML = `<span style="color: #34d399; font-weight:600;">${escapeHtml(diff.actual || "")} ✓ (Passed in ${diff.execution_ms} ms)</span>`;
        } else {
          actualBox.innerHTML = `<span style="color: #f87171; font-weight:600;">${escapeHtml(diff.actual || "")} ✗ (Mismatch)</span>`;
        }
      } else {
        actualBox.innerHTML = '<span style="color: var(--text-dim);">(Click "Run Code" to compile & test solution)</span>';
      }
    }

    function switchDrawerTab(tab) {
      const testPill = document.getElementById('pill-tests');
      const consolePill = document.getElementById('pill-console');
      const testView = document.getElementById('view-sample-tests');
      const consoleView = document.getElementById('view-console');

      if (tab === 'tests') {
        testPill.className = 'nav-pill active';
        consolePill.className = 'nav-pill';
        testView.style.display = 'block';
        consoleView.style.display = 'none';
      } else {
        testPill.className = 'nav-pill';
        consolePill.className = 'nav-pill active';
        testView.style.display = 'none';
        consoleView.style.display = 'block';
      }
    }

    // =========================================================================
    // TIMER & SECURITY HEARTBEAT
    // =========================================================================
    const TOTAL_EXAM_SECONDS = 90 * 60; // 1 hour 30 mins
    let startTime = sessionStorage.getItem('citadel_start_time');
    if (!startTime) {
      startTime = Date.now();
      sessionStorage.setItem('citadel_start_time', startTime);
    } else {
      startTime = parseInt(startTime, 10);
    }

    function updateTimer() {
      const elapsed = Math.floor((Date.now() - startTime) / 1000);
      const remainingSeconds = Math.max(0, TOTAL_EXAM_SECONDS - elapsed);

      const h = String(Math.floor(remainingSeconds / 3600)).padStart(2, '0');
      const m = String(Math.floor((remainingSeconds % 3600) / 60)).padStart(2, '0');
      const s = String(remainingSeconds % 60).padStart(2, '0');

      const timerEl = document.getElementById('timer-display');
      const timerBox = document.getElementById('timer-box');
      if (timerEl) {
        timerEl.innerText = `${h}:${m}:${s}`;
      }

      if (timerBox) {
        if (remainingSeconds <= 300) {
          timerBox.className = "timer-container timer-danger";
        } else if (remainingSeconds <= 600) {
          timerBox.className = "timer-container timer-warning";
        } else {
          timerBox.className = "timer-container";
        }
      }

      if (remainingSeconds === 0) {
        clearInterval(timerInterval);
        autoSubmitTimeUp();
      }
    }

    const timerInterval = setInterval(updateTimer, 1000);
    updateTimer();

    function autoSubmitTimeUp() {
      alert("Exam time has expired! Your code is being submitted automatically.");
      submitFinalCode();
      setTimeout(confirmEndExam, 3000);
    }

    async function confirmEndExam() {
      const elapsed = Math.floor((Date.now() - startTime) / 1000);
      const remainingSeconds = Math.max(0, TOTAL_EXAM_SECONDS - elapsed);
      const isProduction = window.CITADEL_MODE === 'production';

      if (isProduction && remainingSeconds > 0) {
        alert("Minimum sitting duration is 1 hour 30 minutes. You cannot exit early.");
        return;
      }

      const confirmed = confirm("Are you sure you want to conclude and submit your exam session?\\n\\nOnce ended, your session is permanently closed and you cannot log back in.");
      if (!confirmed) return;

      try {
        await fetch('/api/v1/integrity/logout', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            candidate_id: CANDIDATE_ID,
            reason: 'Candidate clicked End Exam and submitted session'
          })
        });
      } catch (e) {}

      document.body.innerHTML = `
        <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100vh;background:#050508;color:#c8cede;font-family:system-ui,sans-serif;text-align:center;padding:24px;">
          <div style="width:64px;height:64px;border-radius:50%;background:rgba(16,185,129,0.12);border:1.5px solid #10b981;display:flex;align-items:center;justify-content:center;font-size:30px;margin-bottom:20px;color:#10b981;">✓</div>
          <h1 style="color:#ffffff;font-size:24px;font-weight:700;margin-bottom:10px;">Exam Concluded Successfully</h1>
          <p style="color:#7a8494;font-size:14px;max-width:540px;line-height:1.6;margin-bottom:24px;">
            Your code submissions and exam telemetry have been permanently finalized and submitted to the evaluation engine.
          </p>
          <div style="background:#0b0d13;border:1px solid #1c2028;border-radius:6px;padding:12px 24px;color:#93c5fd;font-size:12px;font-family:monospace;">
            SESSION STATUS: FINALIZED | Re-entry locked
          </div>
          <p style="color:#3a4252;font-size:12px;margin-top:24px;">Closing secure exam environment and restoring desktop...</p>
        </div>
      `;

      setTimeout(() => {
        window.close();
      }, 1500);
    }

    // Heartbeat every 5s
    setInterval(() => {
      fetch('/api/v1/integrity/heartbeat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          candidate_id: CANDIDATE_ID,
          active_question: 1,
          is_window_focused: document.hasFocus(),
        })
      }).catch(() => {});
    }, 5000);

    window.addEventListener('beforeunload', () => {
      try {
        navigator.sendBeacon('/api/v1/integrity/logout', JSON.stringify({
          candidate_id: CANDIDATE_ID,
          reason: 'Candidate closed exam browser window'
        }));
      } catch(e) {}
    });

    // Anti-cheat input restrictions
    document.addEventListener('contextmenu', e => e.preventDefault());
    window.addEventListener('blur', () => {
      fetch('/api/v1/integrity/event', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          candidate_id: CANDIDATE_ID,
          event_type: 'WINDOW_FOCUS_LOST',
          details: 'Candidate switched window or lost kiosk focus',
          severity: 'HIGH',
        })
      }).catch(() => {});
    });

    // Initialize
    async function init() {
      initAceEditor();

      try {
        const res = await fetch('/api/v1/questions/q1-two-sum');
        if (res.ok) {
          cachedQuestion = await res.json();
          updateCasePills();
          renderCaseDetails(0);
        }
      } catch (e) {}
    }

    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', init);
    } else {
      init();
    }
  