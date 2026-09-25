//! CITADEL Central UI Rendering Engine
//!
//! Provides zero-internet self-contained interfaces:
//! 1. Candidate Portal: HackerRank/LeetCode contest split-screen with real code editor,
//!    canonical Two Sum problem, two-stage evaluation with submit button activation.
//! 2. Protected Recruiter LMS: Administrative management console with candidate monitoring,
//!    one-click disqualification, live metric sync, and question/test case configuration.
//! 3. Admin Access Denied: 403 Forbidden security screen for unauthorized access.
//! 4. Gatekeeper: Candidate download instructions.

pub fn render_portal_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>CITADEL — Code Assessment Portal</title>
  <style>
    :root {
      --bg-base: #0a0e17;
      --bg-surface: #0f172a;
      --bg-surface-elevated: #1e293b;
      --bg-editor: #0d1117;
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

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg-base);
      color: var(--text-main);
      font-family: var(--font-ui);
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      -webkit-font-smoothing: antialiased;
    }

    /* TOP NAVIGATION */
    header {
      height: 52px;
      background: var(--bg-surface);
      border-bottom: 1px solid var(--border-subtle);
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 20px;
      user-select: none;
      flex-shrink: 0;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .logo-badge {
      background: linear-gradient(135deg, #2563eb, #06b6d4);
      color: white;
      font-weight: 900;
      font-size: 13px;
      letter-spacing: 1px;
      padding: 4px 10px;
      border-radius: 6px;
      box-shadow: 0 2px 8px rgba(37, 99, 235, 0.3);
    }

    .exam-title-header {
      font-size: 14px;
      font-weight: 600;
      color: var(--text-main);
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .badge-secure {
      background: rgba(16, 185, 129, 0.12);
      border: 1px solid rgba(16, 185, 129, 0.3);
      color: #34d399;
      font-size: 11px;
      font-weight: 600;
      padding: 2px 8px;
      border-radius: 4px;
    }

    .header-right {
      display: flex;
      align-items: center;
      gap: 16px;
    }

    .timer-container {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 6px;
      padding: 4px 14px;
      display: flex;
      align-items: center;
      gap: 8px;
      font-family: var(--font-mono);
      font-size: 14px;
      font-weight: 700;
      color: #38bdf8;
    }

    /* MAIN CONTEST WORKSPACE (SPLIT VIEW) */
    .workspace {
      display: flex;
      flex: 1;
      height: calc(100vh - 52px);
      overflow: hidden;
    }

    /* LEFT PANE: QUESTION SPEC */
    .pane-problem {
      width: 44%;
      border-right: 1px solid var(--border-subtle);
      background: var(--bg-surface);
      display: flex;
      flex-direction: column;
      overflow-y: auto;
    }

    .problem-tabs {
      display: flex;
      border-bottom: 1px solid var(--border-subtle);
      background: rgba(15, 23, 42, 0.6);
      padding: 8px 16px 0;
      gap: 6px;
    }

    .q-tab-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      padding: 8px 16px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      border-bottom: 2px solid transparent;
      transition: all 0.15s ease;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .q-tab-btn.active {
      color: #38bdf8;
      border-bottom-color: #38bdf8;
      background: rgba(56, 189, 248, 0.06);
      border-radius: 6px 6px 0 0;
    }

    .problem-content {
      padding: 24px;
      overflow-y: auto;
      line-height: 1.6;
    }

    .q-header-meta {
      display: flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 14px;
    }

    .diff-badge {
      font-size: 11px;
      font-weight: 700;
      padding: 3px 10px;
      border-radius: 4px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .diff-easy { background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }
    .diff-medium { background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.3); }
    .diff-hard { background: rgba(239, 68, 68, 0.15); color: #f87171; border: 1px solid rgba(239, 68, 68, 0.3); }

    .points-badge {
      background: rgba(59, 130, 246, 0.15);
      border: 1px solid rgba(59, 130, 246, 0.3);
      color: #60a5fa;
      font-size: 11px;
      font-weight: 600;
      padding: 3px 8px;
      border-radius: 4px;
    }

    .tag-pill {
      background: var(--bg-surface-elevated);
      color: var(--text-dim);
      font-size: 11px;
      padding: 2px 8px;
      border-radius: 12px;
      border: 1px solid var(--border-subtle);
    }

    .q-title {
      font-size: 22px;
      font-weight: 700;
      color: #fff;
      margin-bottom: 16px;
    }

    .q-desc {
      font-size: 14px;
      color: #cbd5e1;
      margin-bottom: 20px;
    }

    .q-desc p { margin-bottom: 10px; }

    .section-head {
      font-size: 12px;
      font-weight: 700;
      color: #94a3b8;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      margin: 20px 0 8px;
    }

    .format-box {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 6px;
      padding: 12px 14px;
      font-size: 13px;
      color: #cbd5e1;
      margin-bottom: 12px;
    }

    .test-card {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 14px;
      margin-bottom: 12px;
    }

    .test-card-title {
      font-size: 12px;
      font-weight: 700;
      color: #38bdf8;
      margin-bottom: 8px;
    }

    .code-snippet {
      background: var(--bg-base);
      border: 1px solid var(--border-subtle);
      border-radius: 4px;
      padding: 8px 12px;
      font-family: var(--font-mono);
      font-size: 13px;
      color: #f1f5f9;
      margin-bottom: 8px;
      white-space: pre-wrap;
    }

    /* RIGHT PANE: CONTEST CODE EDITOR */
    .pane-editor {
      width: 56%;
      display: flex;
      flex-direction: column;
      background: var(--bg-editor);
    }

    .editor-toolbar {
      height: 44px;
      background: #0d131f;
      border-bottom: 1px solid var(--border-subtle);
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 16px;
      user-select: none;
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
      padding: 5px 12px;
      font-size: 13px;
      font-weight: 600;
      outline: none;
      cursor: pointer;
    }

    .toolbar-right {
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .btn-action {
      border: none;
      border-radius: 6px;
      padding: 6px 14px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 6px;
      transition: all 0.15s ease;
    }

    .btn-reset {
      background: transparent;
      color: var(--text-muted);
      border: 1px solid var(--border-subtle);
    }

    .btn-reset:hover {
      background: var(--bg-surface-elevated);
      color: var(--text-main);
    }

    .btn-run {
      background: #2563eb;
      color: white;
      box-shadow: 0 2px 6px rgba(37, 99, 235, 0.4);
    }

    .btn-run:hover {
      background: #1d4ed8;
    }

    /* SUBMIT BUTTON WITH GLOW & LOCK STATE */
    .btn-submit {
      background: #10b981;
      color: white;
      box-shadow: 0 2px 6px rgba(16, 185, 129, 0.4);
    }

    .btn-submit:hover:not(:disabled) {
      background: #059669;
    }

    .btn-submit.locked {
      background: #334155;
      color: #94a3b8;
      box-shadow: none;
      cursor: not-allowed;
      border: 1px dashed rgba(255, 255, 255, 0.15);
    }

    .btn-submit.unlocked {
      background: #10b981;
      color: white;
      animation: pulse-green 2s infinite;
      box-shadow: 0 0 15px rgba(16, 185, 129, 0.6);
      border: 1px solid #34d399;
    }

    @keyframes pulse-green {
      0% { box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
      70% { box-shadow: 0 0 0 10px rgba(16, 185, 129, 0); }
      100% { box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
    }

    /* CODE EDITOR SURFACE */
    .editor-wrapper {
      flex: 1;
      display: flex;
      overflow: hidden;
      position: relative;
      background: #090d16;
    }

    .gutter-lines {
      width: 44px;
      background: #080b12;
      border-right: 1px solid rgba(255, 255, 255, 0.05);
      padding: 14px 6px 14px 0;
      text-align: right;
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.5;
      color: #475569;
      user-select: none;
      overflow: hidden;
      flex-shrink: 0;
    }

    .code-input-area {
      flex: 1;
      width: 100%;
      height: 100%;
      background: transparent;
      border: none;
      outline: none;
      color: #f1f5f9;
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.5;
      padding: 14px 16px;
      resize: none;
      white-space: pre;
      overflow-wrap: normal;
      overflow-x: auto;
      tab-size: 4;
    }

    /* BOTTOM DRAWER: TEST RESULTS & CONSOLE */
    .drawer-console {
      height: 240px;
      background: #090e18;
      border-top: 1px solid var(--border-subtle);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
    }

    .drawer-nav {
      height: 36px;
      background: #0c121e;
      border-bottom: 1px solid var(--border-subtle);
      display: flex;
      align-items: center;
      padding: 0 16px;
      gap: 12px;
      user-select: none;
    }

    .nav-pill {
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      cursor: pointer;
      padding: 4px 8px;
      border-radius: 4px;
      transition: color 0.15s;
    }

    .nav-pill.active {
      color: #38bdf8;
      background: rgba(56, 189, 248, 0.1);
    }

    .drawer-body {
      flex: 1;
      padding: 14px 18px;
      overflow-y: auto;
      font-family: var(--font-mono);
      font-size: 12px;
    }

    .case-pills {
      display: flex;
      gap: 8px;
      margin-bottom: 12px;
    }

    .case-btn {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      color: #cbd5e1;
      padding: 4px 12px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      cursor: pointer;
    }

    .case-btn.active {
      border-color: #38bdf8;
      color: #38bdf8;
      background: rgba(56, 189, 248, 0.1);
    }

    .case-btn.pass { border-color: #10b981; color: #34d399; }
    .case-btn.fail { border-color: #ef4444; color: #f87171; }

    .result-badge {
      display: inline-block;
      padding: 4px 10px;
      border-radius: 4px;
      font-weight: 700;
      font-size: 12px;
      margin-bottom: 8px;
    }

    .badge-accepted { background: rgba(16, 185, 129, 0.2); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.4); }
    .badge-wrong { background: rgba(239, 68, 68, 0.2); color: #f87171; border: 1px solid rgba(239, 68, 68, 0.4); }
    .badge-error { background: rgba(245, 158, 11, 0.2); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.4); }

    /* TOAST ALERT */
    #security-toast {
      display: none;
      position: fixed;
      top: 60px;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(220, 38, 38, 0.95);
      backdrop-filter: blur(8px);
      color: white;
      padding: 10px 22px;
      border-radius: 6px;
      font-size: 13px;
      font-weight: 600;
      box-shadow: 0 8px 24px rgba(0,0,0,0.5);
      z-index: 999999;
      border: 1px solid rgba(248, 113, 113, 0.5);
    }
  </style>
</head>
<body>
  <div id="security-toast">Action blocked by Citadel Security Core</div>

  <!-- HEADER -->
  <header>
    <div class="brand">
      <div class="logo-badge">CITADEL</div>
      <div class="exam-title-header">
        <span>Campus Placement Assessment</span>
        <span class="badge-secure">AIR-GAPPED OFFLINE LAN</span>
      </div>
    </div>
    <div class="header-right">
      <div class="timer-container">
        <span>⏱ Remaining:</span>
        <span id="timer-display">01:30:00</span>
      </div>
    </div>
  </header>

  <!-- WORKSPACE -->
  <div class="workspace">
    <!-- LEFT: PROBLEM SPEC -->
    <div class="pane-problem">
      <div class="problem-tabs" id="question-tabs">
        <button class="q-tab-btn active" id="tab-q1">
          <span>Q1. Two Sum</span>
          <span class="points-badge">100 pts</span>
        </button>
      </div>

      <div class="problem-content" id="problem-view">
        <div class="q-header-meta">
          <span class="diff-badge diff-easy">Easy</span>
          <span class="points-badge">100 Points</span>
          <span class="tag-pill">Array</span>
          <span class="tag-pill">Hash Table</span>
          <span class="tag-pill">Two Pointers</span>
        </div>

        <h1 class="q-title">1. Two Sum</h1>
        <div class="q-desc">
          <p>Given an array of integers <code>nums</code> and an integer <code>target</code>, return <strong>indices of the two numbers</strong> such that they add up to <code>target</code>.</p>
          <p>You may assume that each input would have <strong>exactly one solution</strong>, and you may not use the same element twice.</p>
          <p>You can return the answer in any order (indices separated by space).</p>
        </div>

        <div class="section-head">Input Format</div>
        <div class="format-box">
          The standard input contains space-separated integers representing the array <code>nums</code> followed by the <code>target</code> value as the final integer.<br>
          <em>Example:</em> <code>2 7 11 15 9</code> means <code>nums = [2, 7, 11, 15]</code> and <code>target = 9</code>.
        </div>

        <div class="section-head">Output Format</div>
        <div class="format-box">
          Output the two zero-based indices separated by a space on standard output (e.g. <code>0 1</code>).
        </div>

        <div class="section-head">Constraints</div>
        <div class="format-box" style="line-height: 1.8;">
          • <code>2 &lt;= nums.length &lt;= 10^4</code><br>
          • <code>-10^9 &lt;= nums[i] &lt;= 10^9</code><br>
          • <code>-10^9 &lt;= target &lt;= 10^9</code><br>
          • <strong>Only one valid answer exists.</strong>
        </div>

        <div class="section-head">Sample Cases</div>
        <div class="test-card">
          <div class="test-card-title">Sample Case 1</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Input:</div>
          <div class="code-snippet">2 7 11 15 9</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Expected Output:</div>
          <div class="code-snippet">0 1</div>
          <div style="font-size: 12px; color: #94a3b8;">Explanation: Because nums[0] + nums[1] == 2 + 7 == 9, we return indices 0 1.</div>
        </div>

        <div class="test-card">
          <div class="test-card-title">Sample Case 2</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Input:</div>
          <div class="code-snippet">3 2 4 6</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Expected Output:</div>
          <div class="code-snippet">1 2</div>
          <div style="font-size: 12px; color: #94a3b8;">Explanation: Because nums[1] + nums[2] == 2 + 4 == 6, we return indices 1 2.</div>
        </div>

        <div class="test-card">
          <div class="test-card-title">Sample Case 3</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Input:</div>
          <div class="code-snippet">3 3 6</div>
          <div style="font-size: 11px; color: #94a3b8; margin-bottom: 2px;">Expected Output:</div>
          <div class="code-snippet">0 1</div>
        </div>
      </div>
    </div>

    <!-- RIGHT: CODE EDITOR -->
    <div class="pane-editor">
      <div class="editor-toolbar">
        <div class="toolbar-left">
          <select id="lang-select" class="lang-select" onchange="switchLanguage()">
            <option value="python">Python 3 (v3.13)</option>
            <option value="cpp">C++ 17 (GCC)</option>
            <option value="java">Java 17 (OpenJDK)</option>
          </select>
        </div>
        <div class="toolbar-right">
          <button class="btn-action btn-reset" onclick="resetStarterTemplate()">↺ Reset</button>
          <button class="btn-action btn-run" onclick="runSampleCases()">▶ Run Code</button>
          <button class="btn-action btn-submit locked" id="submit-btn" disabled onclick="submitFinalCode()">
            <span id="lock-icon">🔒</span> <span id="submit-label">Run Sample Cases First</span>
          </button>
        </div>
      </div>

      <div class="editor-wrapper">
        <div class="gutter-lines" id="gutter"></div>
        <textarea id="code-editor" class="code-input-area" spellcheck="false" placeholder="Write your solution here..."></textarea>
      </div>

      <!-- DRAWER -->
      <div class="drawer-console">
        <div class="drawer-nav">
          <div class="nav-pill active" id="pill-tests" onclick="switchDrawerTab('tests')">Sample Test Cases</div>
          <div class="nav-pill" id="pill-console" onclick="switchDrawerTab('console')">Compiler & Execution Output</div>
        </div>
        <div class="drawer-body" id="drawer-content">
          <div id="view-sample-tests">
            <div class="case-pills" id="case-pill-container">
              <button class="case-btn active" onclick="selectCaseView(0)">Case 1</button>
              <button class="case-btn" onclick="selectCaseView(1)">Case 2</button>
              <button class="case-btn" onclick="selectCaseView(2)">Case 3</button>
            </div>
            <div id="case-details-box">
              <div style="color: #94a3b8; font-size: 11px;">Input:</div>
              <div class="code-snippet" id="case-input">2 7 11 15 9</div>
              <div style="color: #94a3b8; font-size: 11px;">Expected Output:</div>
              <div class="code-snippet" id="case-expected">0 1</div>
              <div style="color: #94a3b8; font-size: 11px;">Actual Output:</div>
              <div class="code-snippet" id="case-actual" style="color: #64748b;">(Click "Run Code" to compile & test solution)</div>
            </div>
          </div>
          <div id="view-console" style="display: none; white-space: pre-wrap; color: #94a3b8;">
            (Execution diagnostics will appear here)
          </div>
        </div>
      </div>
    </div>
  </div>

  <script>
    // =========================================================================
    // STARTER TEMPLATES & STATE
    // =========================================================================
    const CANDIDATE_ID = "CAND-" + Math.floor(1000 + Math.random() * 9000);
    let currentLang = "python";
    let samplePassed = false;
    let cachedQuestion = null;
    let latestDiffs = [];
    let activeCaseIdx = 0;

    const STARTERS = {
      python: `import sys

def two_sum(nums, target):
    # Write your solution here
    # Return a list containing the two indices [i, j]
    pass

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
            cout << min(ans[0], ans[1]) << " " << max(ans[0], ans[1]) << "\n";
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
    // CODE EDITOR LOGIC (GUTTER, TABS, INDENTATION)
    // =========================================================================
    const editor = document.getElementById('code-editor');
    const gutter = document.getElementById('gutter');

    function updateLineNumbers() {
      const lines = editor.value.split('\n').length;
      let lineHtml = '';
      for (let i = 1; i <= lines; i++) {
        lineHtml += i + '<br>';
      }
      gutter.innerHTML = lineHtml;
    }

    editor.addEventListener('input', () => {
      updateLineNumbers();
      // Any code modification invalidates prior sample pass
      if (samplePassed) {
        samplePassed = false;
        lockSubmitButton("Code Modified — Re-run Sample Cases");
      }
    });

    editor.addEventListener('scroll', () => {
      gutter.scrollTop = editor.scrollTop;
    });

    // Indentation & Tab key handling
    editor.addEventListener('keydown', function(e) {
      if (e.key === 'Tab') {
        e.preventDefault();
        const start = this.selectionStart;
        const end = this.selectionEnd;
        this.value = this.value.substring(0, start) + "    " + this.value.substring(end);
        this.selectionStart = this.selectionEnd = start + 4;
        updateLineNumbers();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        const start = this.selectionStart;
        const currentLine = this.value.substring(0, start).split('\n').pop();
        const indentMatch = currentLine.match(/^(\s+)/);
        let indent = indentMatch ? indentMatch[1] : '';
        if (currentLine.trim().endsWith(':') || currentLine.trim().endsWith('{')) {
          indent += '    ';
        }
        this.value = this.value.substring(0, start) + '\n' + indent + this.value.substring(this.selectionEnd);
        this.selectionStart = this.selectionEnd = start + 1 + indent.length;
        updateLineNumbers();
      } else if (e.key === '(' || e.key === '[' || e.key === '{' || e.key === '"' || e.key === "'") {
        const pairs = { '(': ')', '[': ']', '{': '}', '"': '"', "'": "'" };
        const closing = pairs[e.key];
        const start = this.selectionStart;
        const end = this.selectionEnd;
        if (closing) {
          e.preventDefault();
          this.value = this.value.substring(0, start) + e.key + closing + this.value.substring(end);
          this.selectionStart = this.selectionEnd = start + 1;
        }
      }
    });

    function switchLanguage() {
      // Save current code
      codeStore[currentLang] = editor.value;
      currentLang = document.getElementById('lang-select').value;
      editor.value = codeStore[currentLang] || STARTERS[currentLang];
      updateLineNumbers();
      samplePassed = false;
      lockSubmitButton("Run Sample Cases First");
    }

    function resetStarterTemplate() {
      if (confirm("Reset current editor code to starter template? Any unsaved edits will be lost.")) {
        editor.value = STARTERS[currentLang];
        updateLineNumbers();
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

    async function runSampleCases() {
      const consoleView = document.getElementById('view-console');
      switchDrawerTab('tests');
      document.getElementById('case-actual').innerHTML = '<span style="color: #38bdf8;">⏳ Compiling and executing on Citadel Judge Sandbox...</span>';

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: 'q1-two-sum',
            language: currentLang,
            source_code: editor.value,
            is_sample_run: true,
            candidate_id: CANDIDATE_ID,
          }),
        });

        const data = await res.json();
        latestDiffs = data.sample_diffs || [];

        // Update pills
        updateCasePills();
        renderCaseDetails(activeCaseIdx);

        if (data.status === 'Accepted') {
          samplePassed = true;
          unlockSubmitButton();
          consoleView.innerHTML = `<span style="color: #34d399;">✓ ACCEPTED — All ${data.passed_cases}/${data.total_cases} sample test cases passed (${data.runtime_ms} ms).\n\nSubmit button is now UNLOCKED. Click 'Submit Solution' to grade against hidden test cases.</span>`;
        } else {
          samplePassed = false;
          lockSubmitButton("Sample Cases Failed");
          consoleView.innerHTML = `<span style="color: #f87171;">✗ ${data.status.toUpperCase()} (${data.passed_cases}/${data.total_cases} Sample Cases Passed)\n\n${data.details}</span>`;
        }
      } catch (err) {
        document.getElementById('case-actual').innerText = "Execution failed: " + err.message;
      }
    }

    async function submitFinalCode() {
      if (!samplePassed) {
        alert("Please run and pass all sample test cases before submitting.");
        return;
      }

      switchDrawerTab('console');
      const consoleView = document.getElementById('view-console');
      consoleView.innerHTML = '<span style="color: #38bdf8;">⏳ Evaluating final submission against all hidden test cases in sandbox...</span>';

      try {
        const res = await fetch('/api/v1/submissions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            question_id: 'q1-two-sum',
            language: currentLang,
            source_code: editor.value,
            is_sample_run: false,
            candidate_id: CANDIDATE_ID,
          }),
        });

        const data = await res.json();
        if (data.status === 'Accepted') {
          consoleView.innerHTML = `<span style="color: #34d399; font-weight: 700; font-size: 14px;">🎉 CONGRATULATIONS! ALL TEST CASES PASSED!</span>\n\n` +
            `• Status: ACCEPTED\n` +
            `• Score Awarded: ${data.score}/100 Points\n` +
            `• Passed Test Cases: ${data.passed_cases}/${data.total_cases} (Sample + Hidden)\n` +
            `• Execution Time: ${data.runtime_ms} ms\n\n` +
            `Your submission has been recorded by the exam proctor server.`;
        } else {
          consoleView.innerHTML = `<span style="color: #f87171; font-weight: 700; font-size: 14px;">✗ ${data.status.toUpperCase()}</span>\n\n` +
            `• Passed: ${data.passed_cases}/${data.total_cases} Test Cases\n` +
            `• Score: ${data.score}/100 Points\n` +
            `• Details: ${data.details}\n\n` +
            `Review your logic, corner cases (negative numbers, duplicates, large bounds) and resubmit.`;
        }
      } catch (err) {
        consoleView.innerHTML = `<span style="color: #ef4444;">Final submission error: ${err.message}</span>`;
      }
    }

    function updateCasePills() {
      const container = document.getElementById('case-pill-container');
      const cases = cachedQuestion ? cachedQuestion.sample_cases : [{input: "2 7 11 15 9", expected_output: "0 1"}, {input: "3 2 4 6", expected_output: "1 2"}, {input: "3 3 6", expected_output: "0 1"}];
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
          actualBox.innerHTML = `<span style="color: #34d399;">${diff.actual} ✓ (Passed in ${diff.execution_ms} ms)</span>`;
        } else {
          actualBox.innerHTML = `<span style="color: #f87171;">${diff.actual} ✗ (Mismatch)</span>`;
        }
      } else {
        actualBox.innerHTML = '<span style="color: #64748b;">(Click "Run Code" to compile & test solution)</span>';
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
    let remainingSeconds = 90 * 60;
    setInterval(() => {
      if (remainingSeconds > 0) {
        remainingSeconds--;
        const h = String(Math.floor(remainingSeconds / 3600)).padStart(2, '0');
        const m = String(Math.floor((remainingSeconds % 3600) / 60)).padStart(2, '0');
        const s = String(remainingSeconds % 60).padStart(2, '0');
        document.getElementById('timer-display').innerText = `${h}:${m}:${s}`;
      }
    }, 1000);

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
      try {
        const res = await fetch('/api/v1/questions/q1-two-sum');
        if (res.ok) {
          cachedQuestion = await res.json();
          updateCasePills();
          renderCaseDetails(0);
        }
      } catch (e) {}
      editor.value = STARTERS.python;
      updateLineNumbers();
    }
    init();
  </script>
</body>
</html>"#
}

pub fn render_recruiter_lms_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>CITADEL LMS — Recruiter & Administrator Center</title>
  <style>
    :root {
      --bg-base: #06090e;
      --bg-surface: #0c121e;
      --bg-surface-elevated: #131b2e;
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
      --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Arial, sans-serif;
      --font-mono: Consolas, monospace;
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
      padding: 16px 28px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .brand-title {
      font-size: 18px;
      font-weight: 700;
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .badge-admin {
      background: rgba(239, 68, 68, 0.15);
      border: 1px solid rgba(239, 68, 68, 0.4);
      color: #f87171;
      font-size: 11px;
      font-weight: 700;
      padding: 3px 10px;
      border-radius: 4px;
      letter-spacing: 0.5px;
    }

    .container {
      max-width: 1300px;
      margin: 0 auto;
      padding: 24px;
      width: 100%;
      flex: 1;
    }

    /* STATS OVERVIEW CARDS */
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(6, 1fr);
      gap: 16px;
      margin-bottom: 24px;
    }

    .stat-card {
      background: var(--bg-surface);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 18px;
    }

    .stat-val {
      font-size: 28px;
      font-weight: 800;
      color: #fff;
      font-family: var(--font-mono);
      margin-top: 6px;
    }

    .stat-label {
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    /* TABS */
    .tabs-bar {
      display: flex;
      gap: 8px;
      border-bottom: 1px solid var(--border-subtle);
      margin-bottom: 24px;
    }

    .tab-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      font-size: 14px;
      font-weight: 600;
      padding: 10px 18px;
      cursor: pointer;
      border-bottom: 2px solid transparent;
    }

    .tab-btn.active {
      color: #38bdf8;
      border-bottom-color: #38bdf8;
      background: rgba(56, 189, 248, 0.05);
    }

    /* TABLES & FORMS */
    .panel-box {
      background: var(--bg-surface);
      border: 1px solid var(--border-subtle);
      border-radius: 8px;
      padding: 20px;
      margin-bottom: 24px;
    }

    .panel-head {
      font-size: 16px;
      font-weight: 700;
      color: #fff;
      margin-bottom: 16px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
    }

    th {
      text-align: left;
      padding: 10px 14px;
      background: rgba(255, 255, 255, 0.03);
      color: var(--text-muted);
      font-weight: 600;
      border-bottom: 1px solid var(--border-subtle);
    }

    td {
      padding: 12px 14px;
      border-bottom: 1px solid var(--border-subtle);
      color: #cbd5e1;
    }

    .status-pill {
      font-size: 11px;
      font-weight: 700;
      padding: 3px 8px;
      border-radius: 4px;
      display: inline-block;
    }

    .status-active { background: rgba(16, 185, 129, 0.15); color: #34d399; }
    .status-flagged { background: rgba(245, 158, 11, 0.15); color: #fbbf24; }
    .status-disqualified { background: rgba(239, 68, 68, 0.15); color: #f87171; }
    .status-logged-out, .status-logged_out { background: rgba(148, 163, 184, 0.15); color: #94a3b8; }

    .btn-table {
      padding: 4px 10px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      border: none;
      cursor: pointer;
    }

    .btn-disqualify { background: #ef4444; color: white; }
    .btn-clear { background: #10b981; color: white; }

    /* INPUT CONTROLS */
    .form-group {
      margin-bottom: 16px;
    }

    .form-group label {
      display: block;
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      margin-bottom: 6px;
    }

    .form-control {
      width: 100%;
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border-subtle);
      border-radius: 6px;
      padding: 8px 12px;
      color: white;
      font-size: 13px;
      outline: none;
    }

    .form-control:focus {
      border-color: #38bdf8;
    }

    .btn-primary {
      background: #2563eb;
      color: white;
      border: none;
      border-radius: 6px;
      padding: 8px 18px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
    }

    .btn-primary:hover {
      background: #1d4ed8;
    }
  </style>
</head>
<body>
  <header>
    <div class="brand-title">
      <span style="color: #38bdf8;">CITADEL LMS</span>
      <span>Recruiter & Administrator Portal</span>
      <span class="badge-admin">CONFIDENTIAL PROCTOR DESK</span>
    </div>
    <div style="font-size: 13px; color: var(--text-muted); display: flex; align-items: center; gap: 8px;">
      <span style="display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #10b981;"></span>
      <span>Live Sync Active (2s interval)</span>
    </div>
  </header>

  <div class="container">
    <!-- STATS -->
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">Total Candidates</div>
        <div class="stat-val" id="stat-total">0</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Active Now</div>
        <div class="stat-val" style="color: #34d399;" id="stat-active">0</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Security Flagged</div>
        <div class="stat-val" style="color: #fbbf24;" id="stat-flagged">0</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Logged Out / Idle</div>
        <div class="stat-val" style="color: #94a3b8;" id="stat-logged-out">0</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Disqualified</div>
        <div class="stat-val" style="color: #f87171;" id="stat-disqualified">0</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Submissions (Pass/Err)</div>
        <div class="stat-val" style="color: #38bdf8;" id="stat-subs">0</div>
      </div>
    </div>

    <!-- TABS -->
    <div class="tabs-bar">
      <button class="tab-btn active" id="tab-candidates" onclick="switchTab('candidates')">Live Candidate Proctoring</button>
      <button class="tab-btn" id="tab-questions" onclick="switchTab('questions')">Question & Test Case Manager</button>
      <button class="tab-btn" id="tab-submissions" onclick="switchTab('submissions')">Submissions & Grading Stream</button>
    </div>

    <!-- TAB 1: CANDIDATES -->
    <div id="view-candidates" class="panel-box">
      <div class="panel-head">Live Monitored Candidate Fleet</div>
      <table>
        <thead>
          <tr>
            <th>Candidate ID</th>
            <th>IP Address</th>
            <th>Score</th>
            <th>Violations</th>
            <th>Last Heartbeat</th>
            <th>Status</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody id="candidates-body">
          <tr><td colspan="7" style="text-align: center; color: var(--text-dim);">Loading live candidates...</td></tr>
        </tbody>
      </table>

      <div class="panel-head" style="margin-top: 32px;">Real-Time Security Integrity Stream</div>
      <table>
        <thead>
          <tr>
            <th>Timestamp</th>
            <th>Candidate</th>
            <th>Violation Event</th>
            <th>Details</th>
            <th>Severity</th>
          </tr>
        </thead>
        <tbody id="violations-body">
          <tr><td colspan="5" style="text-align: center; color: var(--text-dim);">No security violations detected.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- TAB 2: QUESTIONS & TEST CASES -->
    <div id="view-questions" class="panel-box" style="display: none;">
      <div class="panel-head">Configure Assessment Questions (Live In-Memory Bank)</div>
      <form id="question-form" onsubmit="saveQuestion(event)">
        <div class="form-group">
          <label>Problem Title</label>
          <input type="text" class="form-control" id="q-title" value="Two Sum" required>
        </div>
        <div style="display: flex; gap: 16px;">
          <div class="form-group" style="flex: 1;">
            <label>Difficulty</label>
            <select class="form-control" id="q-diff">
              <option value="Easy">Easy</option>
              <option value="Medium">Medium</option>
              <option value="Hard">Hard</option>
            </select>
          </div>
          <div class="form-group" style="flex: 1;">
            <label>Max Points</label>
            <input type="number" class="form-control" id="q-pts" value="100" required>
          </div>
        </div>
        <div class="form-group">
          <label>Problem Description (HTML supported)</label>
          <textarea class="form-control" id="q-desc" rows="4"></textarea>
        </div>
        <div class="form-group">
          <label>Constraints (One per line)</label>
          <textarea class="form-control" id="q-constraints" rows="3"></textarea>
        </div>
        <button type="submit" class="btn-primary">💾 Save Problem Description</button>
      </form>

      <div class="panel-head" style="margin-top: 32px;">Sample Test Cases (Visible to Students)</div>
      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>Input (Standard In)</th>
            <th>Expected Output</th>
            <th>Explanation</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody id="sample-cases-body"></tbody>
      </table>
      <div style="margin-top: 12px; display: flex; gap: 8px;">
        <input type="text" id="new-sample-in" class="form-control" placeholder="Input (e.g. 2 7 11 15 9)">
        <input type="text" id="new-sample-out" class="form-control" placeholder="Expected Output (e.g. 0 1)">
        <input type="text" id="new-sample-exp" class="form-control" placeholder="Explanation (optional)">
        <button class="btn-primary" onclick="addSampleCase()" style="white-space: nowrap;">+ Add Sample</button>
      </div>

      <div class="panel-head" style="margin-top: 32px;">Hidden Test Cases (Server-Only Evaluation)</div>
      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>Input (Standard In)</th>
            <th>Expected Output</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody id="hidden-cases-body"></tbody>
      </table>
      <div style="margin-top: 12px; display: flex; gap: 8px;">
        <input type="text" id="new-hidden-in" class="form-control" placeholder="Hidden Input (e.g. -1 -2 -3 -4 -5 -8)">
        <input type="text" id="new-hidden-out" class="form-control" placeholder="Expected Output (e.g. 2 4)">
        <button class="btn-primary" onclick="addHiddenCase()" style="white-space: nowrap;">+ Add Hidden</button>
      </div>
    </div>

    <!-- TAB 3: SUBMISSIONS -->
    <div id="view-submissions" class="panel-box" style="display: none;">
      <div class="panel-head">Live Candidate Code Submissions Stream</div>
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Candidate</th>
            <th>Language</th>
            <th>Passed</th>
            <th>Score</th>
            <th>Status</th>
            <th>Runtime</th>
          </tr>
        </thead>
        <tbody id="submissions-body">
          <tr><td colspan="7" style="text-align: center; color: var(--text-dim);">No code submissions recorded yet.</td></tr>
        </tbody>
      </table>
    </div>
  </div>

  <script>
    let currentMetrics = null;

    async function fetchMetrics() {
      try {
        const res = await fetch('/api/v1/admin/metrics');
        if (!res.ok) return;
        const data = await res.json();
        currentMetrics = data;

        // Update stats
        document.getElementById('stat-total').innerText = data.total_candidates;
        document.getElementById('stat-active').innerText = data.active_candidates;
        document.getElementById('stat-flagged').innerText = data.flagged_candidates;
        document.getElementById('stat-logged-out').innerText = data.logged_out_candidates;
        document.getElementById('stat-disqualified').innerText = data.disqualified_candidates;
        document.getElementById('stat-subs').innerText = `${data.total_submissions} (${data.passed_submissions_count}✓ / ${data.error_submissions_count}✗)`;

        // Render Candidates
        const candBody = document.getElementById('candidates-body');
        if (data.candidates.length === 0) {
          candBody.innerHTML = '<tr><td colspan="7" style="text-align: center; color: #64748b;">No candidates currently connected.</td></tr>';
        } else {
          candBody.innerHTML = data.candidates.map(c => `
            <tr>
              <td><strong>${c.candidate_id}</strong></td>
              <td><code>${c.ip_address}</code></td>
              <td><span style="color: #60a5fa; font-weight: 700;">${c.total_score} pts</span></td>
              <td><span style="color: ${c.violations_count > 0 ? '#f87171' : '#34d399'}; font-weight: 700;">${c.violations_count}</span></td>
              <td><small>${new Date(c.last_seen).toLocaleTimeString()}</small></td>
              <td><span class="status-pill status-${c.status.toLowerCase().replace(/\s+/g, '-')}">${c.status}</span></td>
              <td>
                ${c.status !== 'Disqualified' ? `<button class="btn-table btn-disqualify" onclick="disqualify('${c.candidate_id}')">Disqualify</button>` : `<button class="btn-table btn-clear" onclick="clearFlag('${c.candidate_id}')">Re-enable</button>`}
              </td>
            </tr>
          `).join('');
        }

        // Render Violations
        const violBody = document.getElementById('violations-body');
        if (data.recent_violations.length === 0) {
          violBody.innerHTML = '<tr><td colspan="5" style="text-align: center; color: #64748b;">No security violations detected.</td></tr>';
        } else {
          violBody.innerHTML = data.recent_violations.slice(0, 20).map(v => `
            <tr>
              <td><small>${new Date(v.timestamp).toLocaleTimeString()}</small></td>
              <td><strong>${v.candidate_id}</strong></td>
              <td><code style="color: #f87171;">${v.event_type}</code></td>
              <td>${v.details}</td>
              <td><span style="color: #fbbf24; font-weight: 600;">${v.severity}</span></td>
            </tr>
          `).join('');
        }

        // Render Submissions
        const subBody = document.getElementById('submissions-body');
        if (data.recent_submissions.length === 0) {
          subBody.innerHTML = '<tr><td colspan="7" style="text-align: center; color: #64748b;">No code submissions recorded yet.</td></tr>';
        } else {
          subBody.innerHTML = data.recent_submissions.slice(0, 30).map(s => `
            <tr>
              <td><small>${new Date(s.timestamp).toLocaleTimeString()}</small></td>
              <td><strong>${s.candidate_id}</strong></td>
              <td><code>${s.language}</code></td>
              <td>${s.passed_cases}/${s.total_cases}</td>
              <td><strong style="color: #34d399;">${s.score} pts</strong></td>
              <td><span class="status-pill ${s.status === 'Accepted' ? 'status-active' : 'status-flagged'}">${s.status}</span></td>
              <td>${s.runtime_ms} ms</td>
            </tr>
          `).join('');
        }

        // Populate question manager if form is idle
        if (data.questions && data.questions[0] && !document.getElementById('q-title').dataset.loaded) {
          const q = data.questions[0];
          document.getElementById('q-title').value = q.title;
          document.getElementById('q-diff').value = q.difficulty;
          document.getElementById('q-pts').value = q.points;
          document.getElementById('q-desc').value = q.description;
          document.getElementById('q-constraints').value = q.constraints.join('\n');
          document.getElementById('q-title').dataset.loaded = "true";
          renderTestCasesTable(q);
        }
      } catch (e) {}
    }

    function renderTestCasesTable(q) {
      const sampleBody = document.getElementById('sample-cases-body');
      sampleBody.innerHTML = q.sample_cases.map((sc, i) => `
        <tr>
          <td>${i + 1}</td>
          <td><code>${sc.input}</code></td>
          <td><code>${sc.expected_output}</code></td>
          <td><small>${sc.explanation || '-'}</small></td>
          <td><button class="btn-table btn-disqualify" onclick="deleteSampleCase(${i})">Delete</button></td>
        </tr>
      `).join('');

      const hiddenBody = document.getElementById('hidden-cases-body');
      hiddenBody.innerHTML = q.hidden_cases.map((hc, i) => `
        <tr>
          <td>${i + 1}</td>
          <td><code>${hc.input}</code></td>
          <td><code>${hc.expected_output}</code></td>
          <td><button class="btn-table btn-disqualify" onclick="deleteHiddenCase(${i})">Delete</button></td>
        </tr>
      `).join('');
    }

    async function disqualify(cid) {
      if (confirm(`Are you sure you want to disqualify candidate ${cid}?`)) {
        await fetch(`/api/v1/admin/candidates/${cid}/disqualify`, { method: 'POST' });
        fetchMetrics();
      }
    }

    async function clearFlag(cid) {
      await fetch(`/api/v1/admin/candidates/${cid}/clear-flag`, { method: 'POST' });
      fetchMetrics();
    }

    async function saveQuestion(e) {
      e.preventDefault();
      await fetch('/api/v1/admin/questions/q1-two-sum', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: document.getElementById('q-title').value,
          difficulty: document.getElementById('q-diff').value,
          points: parseInt(document.getElementById('q-pts').value, 10),
          description: document.getElementById('q-desc').value,
          constraints: document.getElementById('q-constraints').value.split('\n').filter(Boolean),
        })
      });
      alert('Problem specification updated successfully.');
      document.getElementById('q-title').dataset.loaded = "";
      fetchMetrics();
    }

    async function addSampleCase() {
      const inp = document.getElementById('new-sample-in').value.trim();
      const out = document.getElementById('new-sample-out').value.trim();
      const exp = document.getElementById('new-sample-exp').value.trim();
      if (!inp || !out) return alert('Input and Expected Output are required.');

      await fetch('/api/v1/admin/questions/q1-two-sum/sample-cases', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ input: inp, expected_output: out, explanation: exp || null })
      });
      document.getElementById('new-sample-in').value = '';
      document.getElementById('new-sample-out').value = '';
      document.getElementById('new-sample-exp').value = '';
      document.getElementById('q-title').dataset.loaded = "";
      fetchMetrics();
    }

    async function deleteSampleCase(idx) {
      await fetch(`/api/v1/admin/questions/q1-two-sum/sample-cases/${idx}`, { method: 'DELETE' });
      document.getElementById('q-title').dataset.loaded = "";
      fetchMetrics();
    }

    async function addHiddenCase() {
      const inp = document.getElementById('new-hidden-in').value.trim();
      const out = document.getElementById('new-hidden-out').value.trim();
      if (!inp || !out) return alert('Input and Expected Output are required.');

      await fetch('/api/v1/admin/questions/q1-two-sum/hidden-cases', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ input: inp, expected_output: out, explanation: null })
      });
      document.getElementById('new-hidden-in').value = '';
      document.getElementById('new-hidden-out').value = '';
      document.getElementById('q-title').dataset.loaded = "";
      fetchMetrics();
    }

    async function deleteHiddenCase(idx) {
      await fetch(`/api/v1/admin/questions/q1-two-sum/hidden-cases/${idx}`, { method: 'DELETE' });
      document.getElementById('q-title').dataset.loaded = "";
      fetchMetrics();
    }

    function switchTab(t) {
      document.getElementById('tab-candidates').classList.toggle('active', t === 'candidates');
      document.getElementById('tab-questions').classList.toggle('active', t === 'questions');
      document.getElementById('tab-submissions').classList.toggle('active', t === 'submissions');

      document.getElementById('view-candidates').style.display = t === 'candidates' ? 'block' : 'none';
      document.getElementById('view-questions').style.display = t === 'questions' ? 'block' : 'none';
      document.getElementById('view-submissions').style.display = t === 'submissions' ? 'block' : 'none';
    }

    fetchMetrics();
    setInterval(fetchMetrics, 2000);
  </script>
</body>
</html>"#
}

pub fn render_admin_denied_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>403 Forbidden — Administrator Authentication Required</title>
  <style>
    body {
      background: #06090e;
      color: #f8fafc;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      display: flex;
      align-items: center;
      justify-content: center;
      height: 100vh;
      margin: 0;
    }
    .box {
      background: #0f172a;
      border: 1px solid rgba(239, 68, 68, 0.4);
      padding: 36px 40px;
      border-radius: 12px;
      max-width: 520px;
      text-align: center;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }
    h1 { color: #f87171; font-size: 24px; margin-bottom: 12px; }
    p { color: #94a3b8; font-size: 14px; line-height: 1.6; margin-bottom: 24px; }
    .btn {
      display: inline-block;
      background: #2563eb;
      color: white;
      text-decoration: none;
      padding: 10px 20px;
      border-radius: 6px;
      font-weight: 600;
      font-size: 13px;
    }
  </style>
</head>
<body>
  <div class="box">
    <h1>🛡️ Access Denied (403 Forbidden)</h1>
    <p>This administrative learning management & proctoring interface is strictly restricted to authorized recruiters. Student workstations are prohibited from accessing this resource.</p>
    <p style="font-size: 12px; color: #64748b;">Recruiters: Please supply your designated <code>?key=...</code> access token or connect via authorized proctor credential.</p>
    <a href="/" class="btn">Return to Candidate Portal</a>
  </div>
</body>
</html>"#
}

pub fn render_gatekeeper_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>CITADEL Exam Portal — Download Client</title>
  <style>
    body {
      background: #06090e;
      color: #f8fafc;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      margin: 0;
    }
    .card {
      background: #0f172a;
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 12px;
      padding: 40px;
      max-width: 560px;
      text-align: center;
      box-shadow: 0 12px 36px rgba(0,0,0,0.6);
    }
    h1 { font-size: 26px; font-weight: 800; color: #fff; margin-bottom: 12px; }
    p { color: #94a3b8; font-size: 14px; line-height: 1.6; margin-bottom: 24px; }
    .download-btn {
      display: inline-block;
      background: linear-gradient(135deg, #2563eb, #06b6d4);
      color: white;
      text-decoration: none;
      padding: 14px 28px;
      border-radius: 8px;
      font-size: 15px;
      font-weight: 700;
      box-shadow: 0 4px 15px rgba(37, 99, 235, 0.4);
      margin-bottom: 20px;
    }
    .inst {
      text-align: left;
      background: #1e293b;
      padding: 16px 20px;
      border-radius: 8px;
      font-size: 13px;
      color: #cbd5e1;
      line-height: 1.8;
    }
  </style>
</head>
<body>
  <div class="card">
    <h1>CITADEL Assessment Appliance</h1>
    <p>To take this campus recruitment examination, you must launch the secure lockdown client application. It enforces zero-internet isolation and exam environment protection.</p>
    <a href="/download/citadel-client.exe" class="download-btn">⬇ Download CITADEL Client (.exe)</a>
    <div class="inst">
      <strong>Instructions for Candidates:</strong><br>
      1. Download and run <code>citadel-client.exe</code> on your Windows laptop.<br>
      2. Accept the Windows Administrator (UAC) prompt.<br>
      3. The client will automatically secure your workstation and display the Two Sum problem with the code editor.
    </div>
  </div>
</body>
</html>"#
}
