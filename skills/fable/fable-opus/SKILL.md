---
name: fable-opus
description: >
  Run fable-mode execution discipline on Claude Opus — the strongest staged run
  available. Routes the task to the @fable-orchestrator agent (Opus,
  Write-less), which stages the work, delegates artifact production to
  @fable-worker-sonnet / @fable-worker-haiku, and cold-checks deliverables with
  @fable-verifier. Trigger when the user explicitly asks for
  thorough/systematic/"deep work" handling on the strongest model ("fable on
  opus", "stage this on opus", "deep work mode, opus"). Do NOT use for ordinary
  single-pass tasks — and prefer fable-sonnet or fable-haiku when the task
  doesn't need peak reasoning.
---

# Fable Mode — Opus (v3, agent-routed)

v3 change: delegation is enforced structurally, not requested in prose. The
orchestrator is a real agent definition (`agents/fable-orchestrator.md`) with no
Write/Edit tool — it cannot do the work inline, so "spawn a worker" stops being
a suggestion the model can skip. (Change prompted by field report: prose-level
"you may spawn workers" almost always ran inline on the main thread.)

If a task has one obvious correct approach and fits in a single pass, skip this
loop and do it directly.

## How to run it

1. Confirm the fable agents are installed (`fable-orchestrator`,
   `fable-worker-sonnet`, `fable-worker-haiku`, `fable-verifier` appear in the
   available agent types). If they are not, fall back to the inline method:
   spawn a general-purpose Opus agent and pass it the Core loop and operational
   rules verbatim from `agents/fable-orchestrator.md`.
2. Spawn **@fable-orchestrator** via the Task tool (`subagent_type:
   "fable-orchestrator"`). Brief it with: the user's task, the output
   directory, relevant session context, and any user-set limits (warning
   threshold, worker cap, deadline).
3. Do not restate the Core Loop or operational rules in the briefing — the
   orchestrator's agent definition carries them. Brief the task, not the
   method.
4. When it returns, relay the result, every stage it marked unverified, and its
   recommendations (surfaced scope it did not build).

## Known limitation

Write-removal closes the front door, not the side door: the orchestrator keeps
Bash for running verification commands, and Bash can technically create files.
Its definition forbids that use; if audits show it writing through Bash, the
next tightening is removing Bash and routing even check-execution through
workers (cost: one extra hop per check).
