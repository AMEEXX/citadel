# 16 — Implementation Playbook: The Task Contract Standard

## Read this before writing or executing a single task from doc 17 onward

This document is not architecture. It is the **manufacturing standard** every implementation task must be written in, so that a small model or a junior engineer with zero context beyond the task itself can execute it correctly, verify it correctly, and stop instead of guessing when something is unclear. Every future implementation document (17, 18, 19...) is written in this format. If a task doesn't fit the format, the task is too big — split it.

---

## 1. Why this exists (the failure modes it prevents)

Twenty years of watching implementation go wrong teaches you it is never the hard 10% that breaks — the hard 10% gets careful attention. It's the boring 90% where someone (human or model) fills a gap with a guess. Specifically, these six failure modes, in order of how often they actually happen:

| # | Failure mode | What it looks like | The fix baked into this standard |
|---|---|---|---|
| 1 | **Silent scope creep** | Asked to build the login check, the executor also "improves" three other things nearby | Every task has an explicit **Non-goals** list. Touching anything not in **Files touched** is a failure, even if the change is good |
| 2 | **Naming drift** | `SandboxHost` in one file, `sandbox_host`, `Sandbox`, `ExecutionSandbox` in three others — nothing links up at integration time | A single **Glossary** (§4) is the only source of names. A task may not invent a new name for an existing concept |
| 3 | **Silent option-picking** | Task says "use a sandboxing approach"; executor picks whatever it knows, not what the design chose | Every task states the exact decision already made (library, version, flag) as a fact, never as a choice |
| 4 | **Guessing past an unclear spot** | Executor hits an ambiguity and invents a plausible answer rather than flagging it | Rule 8 below: an explicit, mandatory **STOP condition** — a small model is *more* likely to follow this if it's stated as a hard rule with an exact phrase to output, not a vague "ask if unsure" |
| 5 | **"Looks done" instead of "is done"** | Code compiles, executor declares victory, nothing was actually run against the acceptance test | Every task's **Definition of Done** is a command to run and an exact expected output — binary, not subjective |
| 6 | **Forward-referencing something that doesn't exist yet** | Task 12 assumes task 15's output | Tasks are strictly ordered by ID; a task may only depend on a lower-numbered, already-**DONE** task |

---

## 2. The Task Contract — the exact template

Every task, in every implementation document, uses this shape. Nothing is optional; if a section doesn't apply, it says `None` explicitly rather than being omitted (an omitted section is indistinguishable from "forgotten" — an explicit `None` is a decision).

```markdown
### Task <PHASE>-T<n>.<m> — <one-line name>

**Goal (one sentence):** <what exists after this task that didn't before>

**Depends on:** <task IDs that must already show DONE — or "None (first task)">

**Preconditions (verify before starting):**
- <exact command to run> → expect: <exact expected output>
- (repeat for each precondition)

**Exact steps:**
1. <literal, numbered, no room for interpretation — exact file path, exact
   command, exact library name AND version>
2. ...

**Files touched (and ONLY these):**
- `<exact path>` — created / modified / deleted

**Non-goals (do NOT do these, even if they seem related or helpful):**
- <explicit list>

**Definition of Done (all must pass):**
- [ ] `<exact command>` → expect: `<exact output or exit code>`
- [ ] (repeat — every criterion is a command and an expected result, not a feeling)

**Unit test(s) to write:**
- `<test file path>` — <what it asserts, in one sentence per test>

**Functional / integration test(s) to write:**
- `<test file path>` — <the end-to-end behaviour it proves, one sentence>

**Common mistakes here (from real-world experience):**
- <2-4 specific, concrete mistakes people or models make on exactly this kind
  of task — not generic advice>

**If you are unsure about anything in this task, STOP.**
Do not guess. Do not substitute a "reasonable" alternative. Output exactly:
`BLOCKED: <task ID> — <the specific question, in one sentence>`
and wait. A wrong guess that compiles is worse than an honest stop.
```

---

## 3. The ten rules (apply these when *writing* a new task, not just when executing one)

1. **Atomic.** One task = 30 minutes to 3 hours of focused work for a competent engineer. If it's bigger, split it. A task that touches more than ~5 files is almost always too big.
2. **Zero open choices.** Every library, version, flag, port number, file path, and naming decision is stated as a fact. "Pick a suitable JSON library" is a defect in the task, not a normal instruction.
3. **Exact names only, from the Glossary.** Never introduce a new name for something the Glossary already names.
4. **Explicit non-goals on every task.** Not just "what to do" — "what not to also do."
5. **Binary Definition of Done.** Every acceptance line is a command plus an expected result. "Works correctly" is banned language in a Definition of Done.
6. **Strict dependency order.** A task lists exactly which earlier tasks must be `DONE`. No task may assume anything not produced by a `DONE` dependency.
7. **Tests are part of the task, not a follow-up.** A task without its own unit test and, where relevant, its own functional test is incomplete by definition — not "done, tests later."
8. **STOP beats guessing, always.** Every task ends with the literal STOP clause from §2. This is repeated on every single task on purpose — a small model re-reads what's in front of it, not what it read three tasks ago.
9. **No forward references.** If task N needs something task N+3 will build, task N is mis-ordered — renumber, don't work around it.
10. **One reviewer pass per task against the Common-Mistakes QA Gate (§5) before it is marked DONE**, even if all Definition-of-Done checks pass — checks verify the code does what was asked; the QA gate checks the executor didn't *also* do something that wasn't asked.

---

## 4. Canonical glossary (pinned names — do not deviate, do not invent synonyms)

| Canonical name | What it is | Where it's defined |
|---|---|---|
| **Guard** | The privileged lockdown service on the candidate machine | doc 03 §3 |
| `citadel-guard.exe` / `citadel-guardd` | Guard's actual binary name, Windows / Linux | doc 03 §3.4 |
| **Shell** | The kiosk UI process; on Windows, registered as the literal OS shell (doc 15 §6.3) | doc 03 §4 |
| **Forge** | The editor + local run harness inside Shell | doc 03 §5 |
| **SandboxHost** | Guard's subcomponent that executes candidate build/run requests in an isolated, resource-limited context | doc 03 §5.2 |
| **ProcMon** | Guard's process-creation notification/callback module (Windows); the fanotify-based equivalent on Linux is the same concept, same name in docs | doc 03 §3.4 |
| **appliance** / `citadeld` | The server-side binary running all central services | doc 05 |
| **isolate** | The exact third-party sandboxing tool used for judge execution (IOI/CMS sandbox) — never substitute a different sandbox library without a design-doc change | doc 06 |
| **vault** | The sealed hidden-test-case store | doc 06, doc 07 |
| **WAL** | Write-ahead log; every tier (client, edge, appliance) has its own, named `<subject>.wal` | doc 03 §6, doc 09 |
| **.citb** | The encrypted exam bundle file extension | doc 07 |
| **BLAKE3** | The one and only hash function used for content-addressing and integrity checks across the whole system — never MD5/SHA1, never "a hash function," always BLAKE3 | doc 07 |
| **WDAC** | Windows Defender Application Control — the exact mechanism for execution allow-listing on Windows. Not AppLocker. Not a custom allow-list from scratch | doc 03 §3.4 |
| **WFP** | Windows Filtering Platform — the exact mechanism for the Windows network default-deny filter | doc 03 §3.6 |
| **A1–A20** | The numbered attestation checks — always referred to by number, never re-described in prose without the number | doc 03 §3.3, doc 15 §6.1 |
| **M1–M5** | The numbered local-LLM defeat mechanisms — same rule | doc 03 §3.5 |
| **AL1 / AL2 / AL3** | LiveBoot / Managed Lab / BYOD assurance tiers — always by this exact code, never "the strong one" or "the weak one" | doc 01 |

**Rule:** if you need a name that isn't in this table, add it to this table in the same task's Files-touched list (this file gets a line added) — do not use an ad hoc name "just for now."

---

## 5. The Common-Mistakes QA Gate (run this against every completed task)

Before marking any task `DONE`, check it against this list — this is the list a 20-year reviewer actually runs in their head:

- [ ] Did the executor touch any file **not** listed in Files touched? (→ revert the extra change, even if it's an improvement — it belongs in its own task)
- [ ] Did the executor use a library/version different from the one the task specified? (→ this is the single most common small-model failure: substituting a library it "knows better")
- [ ] Are there any hardcoded values (ports, paths, limits) that the design docs specify as configurable or as an exact pinned number? (e.g., rlimit CPU 15s from doc 03 §5.2 — must be the literal number, not "a reasonable timeout")
- [ ] Did the executor write the unit test *after* writing the implementation and shape it to pass, rather than shaping it to the Definition of Done stated in the task? (→ re-derive the test from the Definition of Done, not from the code)
- [ ] Does the task's output use any name not in the Glossary (§4)? (→ fix the name, don't add a synonym)
- [ ] Is there any place the executor filled a gap in the instructions with a guess instead of emitting `BLOCKED: ...`? (→ this is the thing this entire document exists to catch)
- [ ] Does the Definition of Done actually get *run*, with the real output pasted next to the expected output, not just asserted as "passes"?

---

## 6. Task ID scheme (maps 1:1 onto doc 14's roadmap phases — no renumbering across documents)

```
P0-T<n>.<m>   Phase 0 — Lockdown spike           (doc 17)
P1-T<n>.<m>   Phase 1 — Core platform            (doc 18, per workstream, when written)
P2-T<n>.<m>   Phase 2 — Scale and resilience     (doc 19, when written)
P3-T<n>.<m>   Phase 3 — Hardening                (doc 20, when written)
P4-T<n>.<m>   Phase 4 — Commercial readiness     (doc 21, when written)
```
`<n>` = week number from doc 14's table for that phase. `<m>` = task sequence within that week. A task ID is permanent once assigned — if a task is dropped, mark it `WITHDRAWN`, never reuse the number.

---

## 7. Why only Phase 0 is written in full right now

A genuine 20-year instinct, not a shortcut: **writing literal, zero-ambiguity task contracts for all 48 weeks before a single line of code exists is itself a mistake.** Phase 0 exists specifically to test the riskiest assumption (doc 14 §1) — if it fails, Phases 1-4 as currently scoped are wrong anyway, and a fully detailed P1-P4 plan written today would need to be rewritten regardless. The disciplined sequence is:

1. Doc 17 (this standard applied to Phase 0) — written now, in full.
2. Run Phase 0. Its exit criteria (doc 14 §2) either pass or don't.
3. Only once they pass, doc 18 (Phase 1, same standard, same rigor) gets written — informed by what P0 actually revealed, not by what we assumed in September.

This is not "the plan is incomplete." It's "the plan is exactly as complete as it should be at this decision point, and the next document is already specified — its trigger condition is Phase 0's exit criteria passing."
