/**
 * Citadel Unified Restoration Engine (Plan 24)
 * Single authoritative implementation for Workstation Restoration across all candidate states.
 */
(function(window) {
  'use strict';

  let LOCAL_CONTROL_PORT = 8444;

  /**
   * Probe loopback ports 8444..8450.
   * Plan 24 F-2: ONLY accept genuine Citadel client instances (data.app === 'citadel-client').
   * Explicitly rejects 'citadel-supervisor' to prevent stale handoff conflicts.
   */
  async function detectLocalControlPort() {
    for (let p = 8444; p <= 8450; p++) {
      try {
        const controller = new AbortController();
        const tid = setTimeout(() => controller.abort(), 400);
        const res = await fetch(`http://127.0.0.1:${p}/health`, { signal: controller.signal });
        clearTimeout(tid);
        if (res.ok) {
          const data = await res.json();
          if (data && data.app === 'citadel-client') {
            LOCAL_CONTROL_PORT = p;
            return p;
          }
        }
      } catch (e) {
        // Port closed or probe timeout
      }
    }
    return null;
  }

  /**
   * Unified workstation restoration workflow.
   * @param {Object} opts
   * @param {HTMLElement|string} [opts.statusEl] - Element or ID for live badge text
   * @param {boolean} [opts.isForced=false] - Forced exit (e.g. login modal or proctor override)
   * @param {boolean} [opts.isTerminalState=false] - Already ended / disqualified / login phase
   * @param {string} [opts.candidateId] - Candidate ID
   * @param {string} [opts.authToken] - Session Auth Token
   * @param {boolean} [opts.isProduction=false] - Whether in strict production mode
   * @param {number} [opts.remainingSeconds=0] - Remaining exam duration in seconds
   * @param {Function} [opts.onPhase] - Callback when phase updates: (phase, details) => void
   * @param {Function} [opts.onVerified] - Callback when 100% verified: (data) => void
   * @param {Function} [opts.onNoClient] - Callback when no local client was detected
   */
  async function citadelRestore(opts = {}) {
    const statusEl = typeof opts.statusEl === 'string'
      ? document.getElementById(opts.statusEl)
      : (opts.statusEl || document.getElementById('conclusion-status-badge'));

    const cid = opts.candidateId || (typeof CANDIDATE_ID !== 'undefined' ? CANDIDATE_ID : '') || 'terminal-exit';
    let token = opts.authToken || (typeof ACTIVE_AUTH_TOKEN !== 'undefined' ? ACTIVE_AUTH_TOKEN : '');
    if (!token && typeof window !== 'undefined' && window.location) {
      try {
        const p = new URLSearchParams(window.location.search);
        token = p.get('token') || p.get('auth_token') || '';
      } catch (e) {}
    }
    if (!token && typeof sessionStorage !== 'undefined') {
      try {
        token = sessionStorage.getItem('citadel_auth_token') || '';
      } catch (e) {}
    }
    const isProd = typeof opts.isProduction === 'boolean' ? opts.isProduction : (typeof isProductionMode !== 'undefined' ? isProductionMode : false);
    const remaining = typeof opts.remainingSeconds === 'number' ? opts.remainingSeconds : (typeof remainingSeconds !== 'undefined' ? remainingSeconds : 0);
    const isTerminal = opts.isTerminalState || opts.isForced;

    // 1. Enforce 15-minute early exit restriction in production mode during active exam
    if (!isTerminal && isProd && remaining > 900) {
      if (typeof showEarlySubmissionLockedModal === 'function') {
        showEarlySubmissionLockedModal();
      } else {
        alert("Early submission is locked. You can only submit during the final 15 minutes of the exam.");
      }
      return false;
    }

    // Update status element if present
    if (statusEl) {
      statusEl.innerText = "Restoring Workstation... Notifying server & supervisor...";
      statusEl.style.borderColor = "rgba(251, 146, 60, 0.4)";
      statusEl.style.color = "#fed7aa";
    }

    // Plan 25 Channel R1: Concurrently schedule force-restore via handshake token (PRIMARY; works pre-login)
    if (token) {
      try {
        const cForce = new AbortController();
        const tForce = setTimeout(() => cForce.abort(), 800);
        fetch('/api/v1/client/force-restore', {
          method: 'POST',
          signal: cForce.signal,
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            auth_token: token,
            reason: 'Portal Restore Laptop triggered Channel R1'
          })
        }).then(() => clearTimeout(tForce)).catch(() => {});
      } catch (e) {}
    }

    // 2. Concurrently notify central exam server with fast 800ms abort timeouts
    try {
      const c1 = new AbortController();
      const t1 = setTimeout(() => c1.abort(), 800);
      await fetch('/api/v1/client/kill-all-lockdown', {
        method: 'POST',
        signal: c1.signal,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          candidate_id: cid,
          reason: 'Candidate clicked End Exam / Restore Laptop'
        })
      });
      clearTimeout(t1);
    } catch (e) {}

    try {
      const c2 = new AbortController();
      const t2 = setTimeout(() => c2.abort(), 800);
      await fetch('/api/v1/integrity/logout', {
        method: 'POST',
        signal: c2.signal,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          candidate_id: cid,
          reason: 'Candidate clicked End Exam / Restore Laptop'
        })
      });
      clearTimeout(t2);
    } catch (e) {}

    // 3. Probe loopback for genuine citadel-client instance (Plan 24 F-2)
    const port = await detectLocalControlPort();

    if (port) {
      if (statusEl) {
        statusEl.innerText = "Restoring Workstation... Waiting for Supervisor Verification...";
      }

      // 4. Send end-exam trigger to local control (Plan 24 F-4: Status checking + fallback retry)
      let endExamOk = false;
      try {
        const res = await fetch(`http://127.0.0.1:${port}/api/v1/client/end-exam`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            ...(token ? { 'X-Citadel-Auth-Token': token } : {})
          },
          body: JSON.stringify({
            auth_token: token,
            candidate_id: cid,
            already_ended: true,
            reason: 'Web portal Restore Laptop click'
          })
        });

        if (res.ok) {
          endExamOk = true;
        } else if (res.status === 401) {
          // Token mismatch: Retry once without token, relying on loopback already_ended bypass
          const retryRes = await fetch(`http://127.0.0.1:${port}/api/v1/client/end-exam`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              auth_token: '',
              candidate_id: cid,
              already_ended: true,
              reason: 'Web portal Restore Laptop fallback'
            })
          });
          if (retryRes.ok) {
            endExamOk = true;
          }
        }
      } catch (e) {
        // Network error contacting client (may have already terminated)
      }

      // 5. Poll supervisor restoration status on loopback (Plan 24 F-3: 90s timeout window)
      const pollStart = Date.now();
      const maxPollMs = 90000; // 90 seconds
      let isVerified = false;
      let lastPhase = "";

      while (Date.now() - pollStart < maxPollMs) {
        await new Promise(r => setTimeout(r, 600));
        try {
          const controller = new AbortController();
          const tId = setTimeout(() => controller.abort(), 1200);
          const res = await fetch(`http://127.0.0.1:${port}/restore-status`, {
            signal: controller.signal
          });
          clearTimeout(tId);

          if (res.ok) {
            const data = await res.json();
            if (data) {
              if (data.phase && data.phase !== lastPhase) {
                lastPhase = data.phase;
                if (statusEl) {
                  const phaseLabels = {
                    spawning: "supervisor initializing",
                    killing: "terminating client processes",
                    registry: "resetting registry policies",
                    taskbars: "restoring taskbars",
                    services: "re-enabling services",
                    explorer: "restarting Explorer shell",
                    touchpad: "restoring touchpad gestures",
                    verifying: "verifying system state"
                  };
                  const label = phaseLabels[data.phase] || data.phase;
                  statusEl.innerText = `Restoring workstation (${label})...`;
                }
                if (typeof opts.onPhase === 'function') {
                  opts.onPhase(data.phase, data.details || '');
                }
              }

              if (data.verified === true) {
                isVerified = true;
                if (typeof opts.onVerified === 'function') {
                  opts.onVerified(data);
                }
                break;
              }
            }
          }
        } catch (e) {
          // Tolerate handoff gap between client dropping port and supervisor binding port
        }
      }

      if (statusEl) {
        if (isVerified) {
          statusEl.innerText = "All Citadel processes destroyed \u2014 Verified 100% Restored.";
          statusEl.style.borderColor = "#38a169";
          statusEl.style.color = "#86efac";
        } else {
          statusEl.innerText = "Restoration Incomplete \u2014 Press Ctrl+Shift+Alt+Q or run RESTORE_MY_LAPTOP.bat";
          statusEl.style.borderColor = "#ef4444";
          statusEl.style.color = "#fca5a5";
        }
      }
      return isVerified;
    } else {
      // Channel R1 might have just been received by the client; wait a few seconds to see if supervisor binds
      let supPort = null;
      for (let attempt = 0; attempt < 8; attempt++) {
        await new Promise(r => setTimeout(r, 600));
        for (let p = 8444; p <= 8450; p++) {
          try {
            const controller = new AbortController();
            const tid = setTimeout(() => controller.abort(), 300);
            const res = await fetch(`http://127.0.0.1:${p}/restore-status`, { signal: controller.signal });
            clearTimeout(tid);
            if (res.ok) {
              supPort = p;
              break;
            }
          } catch(e) {}
        }
        if (supPort) break;
      }

      if (supPort) {
        // Supervisor came up via Channel R1! Poll supervisor status directly:
        const pollStart = Date.now();
        const maxPollMs = 90000;
        let isVerified = false;
        let lastPhase = "";

        while (Date.now() - pollStart < maxPollMs) {
          await new Promise(r => setTimeout(r, 600));
          try {
            const controller = new AbortController();
            const tId = setTimeout(() => controller.abort(), 1200);
            const res = await fetch(`http://127.0.0.1:${supPort}/restore-status`, { signal: controller.signal });
            clearTimeout(tId);
            if (res.ok) {
              const data = await res.json();
              if (data) {
                if (data.phase && data.phase !== lastPhase) {
                  lastPhase = data.phase;
                  if (statusEl) {
                    statusEl.innerText = `Restoring workstation (${data.phase})...`;
                  }
                }
                if (data.verified === true) {
                  isVerified = true;
                  break;
                }
              }
            }
          } catch(e) {}
        }

        if (statusEl) {
          if (isVerified) {
            statusEl.innerText = "All Citadel processes destroyed \u2014 Verified 100% Restored.";
            statusEl.style.borderColor = "#38a169";
            statusEl.style.color = "#86efac";
          } else {
            statusEl.innerText = "Restoration Incomplete \u2014 Press Ctrl+Shift+Alt+Q or run RESTORE_MY_LAPTOP.bat";
            statusEl.style.borderColor = "#ef4444";
            statusEl.style.color = "#fca5a5";
          }
        }
        return isVerified;
      }

      // Truly no client & no supervisor:
      if (statusEl) {
        statusEl.innerText = "No Citadel lockdown detected on this workstation \u2014 workstation already restored.";
        statusEl.style.borderColor = "#38a169";
        statusEl.style.color = "#86efac";
      }

      // Ensure manual fallback download link is shown if available
      const fallbackContainer = document.getElementById('manual-restore-fallback')
        || document.getElementById('no-client-restore-card');
      if (fallbackContainer) {
        fallbackContainer.style.display = 'block';
      }

      if (typeof opts.onNoClient === 'function') {
        opts.onNoClient();
      }
      return true;
    }
  }

  // Export to window
  window.detectLocalControlPort = detectLocalControlPort;
  window.citadelRestore = citadelRestore;

})(window);
