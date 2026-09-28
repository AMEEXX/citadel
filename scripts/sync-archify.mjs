import { execSync } from 'child_process';
import { writeFileSync, copyFileSync, existsSync, mkdirSync } from 'fs';
import { resolve, join } from 'path';

const projectRoot = resolve('.');
const archifyBin = join(projectRoot, '.agents/skills/archify/bin/archify.mjs');

console.log('=== Citadel Archify Synchronization Pipeline ===');

let gitCommit = '35d8d3cfccc154589d1e5187dfb11b24e59dadeb';
let gitRemote = 'https://github.com/AMEEXX/citadel.git';

try {
  gitCommit = execSync('git rev-parse HEAD', { encoding: 'utf8' }).trim();
} catch (e) {}

try {
  gitRemote = execSync('git remote get-url origin', { encoding: 'utf8' }).trim();
} catch (e) {}

console.log('Pinned Git Commit:', gitCommit);
console.log('Pinned Git Remote:', gitRemote);

const candidate = {
  schema_version: 1,
  diagram_type: 'architecture',
  meta: {
    title: 'Citadel High-Assurance Lockdown and Exam Platform',
    subtitle: 'Air-gapped Win32 Kiosk Lockdown Client, Kernel WFP Net Isolation and Axum SSE LAN Server',
    output: '.archify/citadel-architecture.html',
    visual_preset: 'signal-flow',
    animation: 'trace',
    quality_profile: 'showcase',
    viewBox: [1060, 620],
    repository: {
      url: gitRemote,
      revision: gitCommit.length === 40 ? gitCommit : '35d8d3cfccc154589d1e5187dfb11b24e59dadeb',
      provider: 'github',
      link_mode: 'web'
    },
    views: [
      {
        id: 'student-journey',
        label: 'Candidate Lockdown Flow',
        focus: ['candidate', 'kiosk_client', 'hotkey_lock', 'portal_ui', 'citadel_server'],
        note: 'Traces student login, Secure Desktop isolation, keyboard lockdown and heartbeat stream.'
      },
      {
        id: 'proctor-oversight',
        label: 'Real-Time Proctor Stream',
        focus: ['portal_ui', 'citadel_server', 'sse_stream', 'recruiter_ui', 'proctor'],
        note: 'Traces violation events from kiosk browser via server to real-time proctor dashboard.'
      },
      {
        id: 'security-boundary',
        label: 'Security and Anti-Cheat Mesh',
        focus: ['hotkey_lock', 'guard_net', 'kiosk_client', 'citadel_server'],
        note: 'Highlights low-level Win32 hooks and WFP kernel network traffic containment.'
      }
    ]
  },
  components: [
    {
      id: 'candidate',
      type: 'external',
      label: 'Candidate',
      sublabel: 'Student at Workstation',
      pos: [50, 80],
      size: [140, 60],
      icon: 'person'
    },
    {
      id: 'kiosk_client',
      type: 'security',
      label: 'Kiosk Client',
      sublabel: 'Citadel Win32 Shell',
      pos: [50, 220],
      size: [140, 60],
      tag: 'Secure Desktop',
      sources: [
        { path: 'citadel-client/src/main.rs', line: 1, label: 'Client Entry' },
        { path: 'citadel-client/src/kiosk_window.rs', line: 1, label: 'Kiosk Window' }
      ]
    },
    {
      id: 'hotkey_lock',
      type: 'security',
      label: 'Hotkey Lock',
      sublabel: 'LL Keyboard Hook',
      pos: [50, 360],
      size: [140, 60],
      tag: 'Alt+Tab / WinKeys',
      sources: [
        { path: 'citadel-client/src/hotkey_lock.rs', line: 1, label: 'Keyboard Hook' }
      ]
    },
    {
      id: 'portal_ui',
      type: 'frontend',
      label: 'Student Portal UI',
      sublabel: 'portal.html + Ace/Monaco',
      pos: [390, 80],
      size: [150, 60],
      tag: 'Exam Engine',
      sources: [
        { path: 'citadel-server/templates/portal.html', line: 1, label: 'Portal Template' }
      ]
    },
    {
      id: 'recruiter_ui',
      type: 'frontend',
      label: 'Recruiter Console',
      sublabel: 'recruiter.html Dashboard',
      pos: [390, 220],
      size: [150, 60],
      tag: 'Live Proctoring',
      sources: [
        { path: 'citadel-server/templates/recruiter.html', line: 1, label: 'Recruiter LMS' }
      ]
    },
    {
      id: 'proctor',
      type: 'external',
      label: 'Recruiter / Proctor',
      sublabel: 'Exam Administrator',
      pos: [390, 360],
      size: [150, 60],
      icon: 'briefcase'
    },
    {
      id: 'guard_net',
      type: 'security',
      label: 'Guard-Net (WFP)',
      sublabel: 'Kernel Filtering Engine',
      pos: [390, 490],
      size: [150, 60],
      tag: 'LAN-Only Isolation',
      sources: [
        { path: 'guard-net/Cargo.toml', line: 1, label: 'Guard-Net Crate' }
      ]
    },
    {
      id: 'citadel_server',
      type: 'backend',
      label: 'Citadel Server',
      sublabel: 'Axum REST and SSE Hub',
      pos: [740, 80],
      size: [160, 60],
      tag: 'LAN Port 8080',
      sources: [
        { path: 'citadel-server/src/main.rs', line: 1, label: 'Server Main' },
        { path: 'citadel-server/src/api.rs', line: 518, label: 'API Router' }
      ]
    },
    {
      id: 'sse_stream',
      type: 'messagebus',
      label: 'Live Event Stream',
      sublabel: 'SSE /api/events',
      pos: [740, 220],
      size: [160, 60],
      tag: 'Violation Telemetry',
      sources: [
        { path: 'citadel-server/src/api.rs', line: 518, label: 'SSE Handler' }
      ]
    },
    {
      id: 'sqlite_db',
      type: 'database',
      label: 'Persistence Store',
      sublabel: 'SQLite and InMemory State',
      pos: [740, 360],
      size: [160, 60],
      tag: 'Submissions and State',
      sources: [
        { path: 'citadel-server/src/api.rs', line: 256, label: 'AppState Engine' }
      ]
    }
  ],
  boundaries: [
    {
      kind: 'security-group',
      label: 'Student Workstation (Lockdown Enclave)',
      wraps: ['candidate', 'kiosk_client', 'hotkey_lock']
    },
    {
      kind: 'security-group',
      label: 'Proctoring and Oversight Operations',
      wraps: ['proctor', 'recruiter_ui']
    },
    {
      kind: 'region',
      label: 'Citadel LAN Appliance and Server Infrastructure',
      wraps: ['portal_ui', 'guard_net', 'citadel_server', 'sqlite_db', 'sse_stream']
    }
  ],
  connections: [
    {
      id: 'candidate-to-kiosk',
      from: 'candidate',
      to: 'kiosk_client',
      label: 'operates',
      variant: 'emphasis'
    },
    {
      id: 'kiosk-to-hotkey',
      from: 'kiosk_client',
      to: 'hotkey_lock',
      label: 'attaches hook',
      variant: 'security'
    },
    {
      id: 'kiosk-to-portal',
      from: 'kiosk_client',
      to: 'portal_ui',
      label: 'hosts Chromium kiosk',
      variant: 'default',
      fromSide: 'right',
      toSide: 'left'
    },
    {
      id: 'portal-to-server',
      from: 'portal_ui',
      to: 'citadel_server',
      label: 'REST /api/submit',
      variant: 'emphasis'
    },
    {
      id: 'server-to-sse',
      from: 'citadel_server',
      to: 'sse_stream',
      label: 'emits events',
      variant: 'emphasis'
    },
    {
      id: 'sse-to-recruiter',
      from: 'sse_stream',
      to: 'recruiter_ui',
      label: 'SSE proctor stream',
      variant: 'emphasis',
      fromSide: 'left',
      toSide: 'right'
    },
    {
      id: 'server-to-db',
      from: 'citadel_server',
      to: 'sqlite_db',
      label: 'writes transactions',
      variant: 'default',
      fromSide: 'right',
      toSide: 'right'
    },
    {
      id: 'proctor-to-recruiter',
      from: 'proctor',
      to: 'recruiter_ui',
      label: 'monitors roster',
      variant: 'default'
    },
    {
      id: 'guardnet-to-server',
      from: 'guard_net',
      to: 'citadel_server',
      label: 'isolates LAN',
      variant: 'security',
      fromSide: 'right',
      toSide: 'bottom'
    }
  ],
  cards: [
    {
      dot: 'rose',
      title: 'Hardened Kiosk Enclave',
      items: [
        'Win32 lpDesktop isolation switches candidate to Secure Desktop plane',
        'WH_KEYBOARD_LL hook blocks Alt+Tab, Windows keys, and task switching',
        'Active watchdog detects blacklisted processes, multi-monitors, and debuggers'
      ]
    },
    {
      dot: 'cyan',
      title: 'Air-Gapped LAN Communications',
      items: [
        'Kernel WFP engine isolates workstation traffic exclusively to the Citadel LAN appliance',
        'Axum HTTP server handles low-latency exam submission and code execution',
        'Dual-engine state management with SQLite persistence and atomic in-memory fallback'
      ]
    },
    {
      dot: 'emerald',
      title: 'Real-Time Integrity and Proctoring',
      items: [
        'Student portal sends continuous heartbeat and window focus telemetry every 2.5s',
        'Server-Sent Events (SSE) stream instant violation alerts to recruiter dashboard',
        'Recruiter LMS console tracks live candidate state, test progress, and lockdown posture'
      ]
    }
  ]
};

mkdirSync(join(projectRoot, '.archify'), { recursive: true });
const candidatePath = join(projectRoot, '.archify/citadel-architecture.candidate.json');
writeFileSync(candidatePath, JSON.stringify(candidate, null, 2), 'utf8');
console.log('Candidate written to:', candidatePath);

const htmlPath = join(projectRoot, '.archify/citadel-architecture.html');
const staticHtmlPath = join(projectRoot, 'citadel-server/static/citadel-architecture.html');

console.log('Compiling Archify diagram with showcase quality...');
try {
  execSync(`node "${archifyBin}" render architecture "${candidatePath}" "${htmlPath}" --quality showcase --repo-root "${projectRoot}"`, {
    stdio: 'inherit'
  });
  console.log('Diagram successfully rendered to:', htmlPath);

  mkdirSync(join(projectRoot, 'citadel-server/static'), { recursive: true });
  copyFileSync(htmlPath, staticHtmlPath);
  console.log('Copied to Citadel server static assets:', staticHtmlPath);
  console.log('Sync complete.');
} catch (e) {
  console.error('Archify render error:', e.message);
  process.exit(1);
}
