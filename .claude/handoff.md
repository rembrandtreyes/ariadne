# Handoff — ariadne — 2026-07-18

## Mission
Fix the five substrate gaps that lost M0 Gate 2 (agent+ariadne 7.5/10 vs agent+ripgrep
10.0/10 on a 10-task blinded eval), then re-run that exact eval. M1 `ariadne drift` is
HALTED by ISA D-18 until the graph beats the real substitute; every gap is deterministic
parser work, so this is in-moat. The five gaps, in suggested order:
1. Discovery honors `.gitignore` (PathFilter in `src/pipeline/discovery.rs` — it indexed
   4 gitignored `.claude/worktrees/*` repo copies on a real fixture; 3,847 files vs 792)
2. tsconfig path-alias resolution (`@/…`) in `src/pipeline/import_resolution.rs` — its
   absence blinds imports/boundaries/check-rules on modern TS repos (the target market)
3. Symbol-name collisions: name→symbol resolution silently picks lowest id — disambiguate
   (import-guided / same-file / path-proximity) or surface ambiguity instead of guessing
4. Missing reference edges: aliased refs (`const x = fn`), const-only exports, type-only
   imports — caller lists are floors today (missed 2 of 18 real referencing files)
5. Route→lib edge gaps: route handlers' calls into lib sometimes don't land, producing
   false "test-only caller" verdicts (repro: timbre `analyzeVoice`)

## Definition of Done
- Each gap fixed with a red-first regression test; full suite (34 binaries), clippy, fmt green
- Determinism holds: 3 consecutive full indexes of one repo → byte-identical communities/
  blast-radius/cycles output (regression tests from e205a30 stay green)
- Eval re-run per the archived protocol on a FRESH frozen timbre copy, fresh blinded arms,
  fresh adjudication; verdict row appended to strategy ISA (D-19): GO only if the ariadne
  arm wins structure questions without losing staleness — an honest repeat NO-GO also closes
- Adjudication key kept in a path no arm ever reads (see Gotchas)

## Where Things Stand
- M0 complete in strategy ISA (27/27): Gate 1 determinism GO (hash-order class swept, 6
  fixes, commits 3035227 + e205a30 + 48747a4), Gate 3 pain GO (30 HN quotes, 15
  buyer-language), Gate 2 eval NO-GO — full per-question adjudication + product-gap
  detail live in gate2-eval-scorecard.md next to the ISA
- All three commits are on local main, UNPUSHED — push only if Rembrandt says so
- Working tree is clean; the eval fixture + arms lived in session scratchpad and are GONE —
  rebuild the fixture by rsyncing `~/loremllc/timbre` (exclude node_modules/.git/.next/.env*)
- Eval method note: arms used the ariadne CLI as the MCP proxy (same engine); recorded in ISA
- crates.io rename still a blocker (name squatted); cross-vendor audit capability still zero

## Gotchas — don't re-lose this time
- Scoring-key leak: file-change notices broadcast edits to any agent that previously Read
  the file — during evals, keep ground-truth/scoring files in a directory arms never touch
- Until gap 1 lands, any real-repo fixture MUST set `exclude=[".claude", …]` in ariadne.toml
  or the index is polluted by worktrees; timbre also has scripts/generate-detector-samples.ts
  which locally re-implements lib symbols — perfect collision fixture for gap 3 tests
- Reddit's public JSON API 403-blocks this IP outright (80/80 requests) — use HN Algolia
- ariadne's `imports` table only resolves relative imports today — don't trust boundaries/
  check-rules output on alias-based codebases until gap 2 is fixed
- Renames leave dangling test imports invisible to the call graph (rg caught one on timbre) —
  a good extra edge-type test case for gap 4

## Read First
- `~/.claude/LIFEOS/MEMORY/WORK/ariadne-pivot-strategy/gate2-eval-scorecard.md` — gap detail + adjudication
- `~/.claude/LIFEOS/MEMORY/WORK/ariadne-pivot-strategy/gate2-eval-protocol.md` — reusable eval design
- `~/.claude/LIFEOS/MEMORY/WORK/ariadne-pivot-strategy/ISA.md` — D-16..D-18, the fork this mission resolves
- `src/pipeline/discovery.rs` — PathFilter, gap 1 home
- `src/pipeline/import_resolution.rs` — alias resolution, gap 2 home
- `CLAUDE.md` — commands, structure, conventions

Start by reading the files above, then begin the Mission.
