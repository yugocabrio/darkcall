# Development instructions

Work in /Users/yugo/zzzzzzcash on DarkCall for ZECATHON.
The human authorized implementation and regular commits/pushes to
git@github.com:yugocabrio/ZECATHON.git on October 4, 2026.

## Continuing work

1. Read the latest human messages, STATUS.md, TASKS.md and docs/protocol.md.
2. Inspect git status and any recorded running process before choosing work.
3. Continue the highest-priority unfinished task. Finishing one task is not the end
   of the project; select the next useful task.
4. Implement, run relevant checks, preserve measurements and errors, and update
   STATUS.md and TASKS.md. Do not substitute documentation for implementation.
5. Commit coherent changes and push to origin/main when checks appropriate to the
   change pass. A useful failed experiment may also be committed with its failure
   explicitly recorded. Never force-push or erase human changes.
6. If blocked, preserve the concrete evidence and advance an independent task.
   Report a material blocker once; do not repeat an unchanged failure indefinitely.

The 15-minute heartbeat is recovery only. The human explicitly asked to keep
working as far as possible and not split active work into artificial 15-minute
turns. Continue sustained implementation across useful milestones; do not end a
turn merely because one queue item or a timer interval is complete.

## Boundaries and truthfulness

- Latest direct human instructions override this queue. Honor any pause immediately.
- Use testnet or offline synthetic fixtures. No mainnet funds or real-money custody.
- Keep the repository private until the human authorizes changing visibility.
- No external messages, contest submission or deployment without authorization.
- Never commit keys, wallet databases, PCZTs, witness files, real bids or secret logs.
- Avoid use of employer proprietary code or confidential information.
- Reference plaintext fixtures are correctness oracles, not privacy demonstrations.
- Do not claim malicious MPC security from a semi-honest implementation.
- Pin cryptographic dependencies; record versions, licenses and actual commands.
- Do not silently replace native output-binding proofs with operator attestation.
  Such a guarantee change needs a human review.
- Do not change the product to ZSA swaps or another auction type without review.
- Do not create other chats or delegate to subagents without direct human authorization.

## Deadlines

- October 4–6: smallest actual proofs and native signing experiments.
- October 7–12: four-bid integrated flow.
- October 13–19: failure recovery, adversarial cases and public verification.
- October 20–24: UI, independent reproduction, benchmarks and demo.
- October 25–28: feature freeze, fixes and submission package.
- Submission deadline: October 28, 2026 23:59 UTC.

Dates are targets, not a substitute for demonstrated results. If the deadline or
feature-freeze date is reached, prioritize completion and report remaining gaps.
