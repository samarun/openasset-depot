# Testing the host integrations

Nine creative applications integrate with OpenAsset Depot. Most of them are
commercial software behind a licence server, which means no hosted CI runner can
install them, which for a long time meant the only integration ever exercised in
a real host was whichever one the developer had open. Breakage reached studios
before it reached us.

This page describes how each host is actually covered, and how a studio can
attach its own licensed runners so its versions are covered too.

## What runs where

| Host | Covered by | Where it runs |
| --- | --- | --- |
| Blender | `scripts/host-smoke.py --host blender` | Every push, on GitHub-hosted runners, against Blender 3.6 and 4.2 |
| Maya | `scripts/host-smoke.py --host maya` | Self-hosted runner labelled `oad-host-maya` |
| Houdini | `scripts/host-smoke.py --host houdini` | Self-hosted runner labelled `oad-host-houdini` |
| Nuke | `scripts/host-smoke.py --host nuke` | Self-hosted runner labelled `oad-host-nuke` |
| Adobe, Resolve | `node --check`, shared word-list parity, packaging | Every push |
| Unity, Unreal | Word-list and palette parity, packaging | Every push |

Blender is free, so it is the leg that runs on every push and is the early
warning for anything that breaks the shared Python bridge. Because all four
Python hosts share `plugins/common`, a Blender failure usually means Maya,
Houdini, and Nuke are broken too.

Adobe, Resolve, Unity, and Unreal have no batch mode worth driving from CI. Their
panels are checked for syntax, and their hand-copied word lists and palettes are
checked against the Python source of truth by
`plugins/common/tests/test_words.py`, which is where those copies would otherwise
drift.

## Running a host test locally

```bash
./scripts/host-smoke.py --list          # what can be found on this machine
./scripts/host-smoke.py                 # every host that is installed
./scripts/host-smoke.py --host nuke     # one host
```

The driver finds each application on `PATH` and in the usual install locations
for macOS, Linux, and Windows, preferring the newest version it finds. Point it
somewhere specific with an environment variable when that guess is wrong:

```bash
OAD_MAYA_EXECUTABLE=/opt/autodesk/maya2025/bin/mayapy ./scripts/host-smoke.py --host maya
```

Exit codes distinguish the three outcomes that matter:

| Code | Meaning |
| --- | --- |
| 0 | The host ran the smoke test and it passed |
| 1 | The test failed, the host crashed, or `--require-host` was set and the host is missing |
| 77 | The host is not installed here, so nothing was verified |

A skip is deliberately not a pass. CI legs pass `--require-host` so a runner
whose licence server has gone away fails loudly instead of quietly verifying
nothing.

## What the smoke tests check

Each test drives the host's own panel inside the real application against a fake
bridge, so no server or database is needed — the API is covered separately by
the integration suite. What only reproduces inside the host is:

- **The panel loads.** Every host ships its own Python and, for Houdini, its own
  Qt. An import that works in one Houdini build and not another only shows up
  under `hython`.
- **Worker results come back.** Each host marshals background results onto its
  main thread differently (`maya.utils.executeDeferred`, Blender's timer,
  `nuke.executeInMainThread`). A panel that works interactively and hangs in
  batch is nearly always this.
- **Labels are canonical.** Buttons must come from the shared word list, so a
  host cannot reintroduce its own vocabulary. See [the glossary](glossary.md).

`scripts/host-smoke/harness.py` holds the fake bridge and the shared
expectations. Adding a host means writing the twenty host-specific lines, not a
new test suite.

## Attaching a licensed runner

1. Provision a machine that has the host installed and can reach your licence
   server. A workstation is fine; these tests take seconds.
2. Install the GitHub Actions runner on it and give it two labels:
   `self-hosted` and `oad-host-<name>`, where `<name>` is `maya`, `houdini`, or
   `nuke`. One machine can carry several host labels.
3. Confirm the driver can find the application on that machine:

   ```bash
   ./scripts/host-smoke.py --list
   ```

   If a host shows as not installed, set its `OAD_<HOST>_EXECUTABLE` variable in
   the runner's environment.
4. Set the repository variable `OAD_LICENSED_HOST_RUNNERS` to `true`. The
   licensed matrix legs in `.github/workflows/host-matrix.yml` are skipped
   entirely unless this is set, so a fork with no runners still gets a green
   build instead of jobs that queue forever.

Houdini Apprentice and Nuke Non-commercial are sufficient for these tests; the
integrations do not use anything gated behind a commercial licence, and running
against the free tiers is a useful check that they stay that way.

### Covering several application versions

Studios rarely run one version. Give each machine its own label suffix and add
matrix entries, or point `OAD_<HOST>_EXECUTABLE` at a specific install per
runner. The Blender leg already does this with a version matrix, which is the
pattern to copy.

## Timeouts

A host that cannot get a licence usually blocks rather than exits. The driver
kills a run after ten minutes; override with `OAD_HOST_SMOKE_TIMEOUT` (seconds)
on a slow or heavily loaded runner.
