# Architecture decision records

Each file records an important decision: its context, what was decided, the alternatives and what it implies. It's there so nobody undoes a decision without knowing why it was made.

Write an ADR only when the decision meets all three:

1. **It's hard to undo** (changing it later is costly).
2. **It would surprise someone without context** ("why like this?").
3. **There were real alternatives**, and one was picked for concrete reasons.

ADRs aren't edited once accepted (translations and status links aside). If a decision changes, write a new one saying "Supersedes 000X" and mark the old one as superseded.

| # | Decision | Status |
|---|---|---|
| [0001](0001-web-ui-in-webview2.md) | The interface is a web page in WebView2 | Accepted |
| [0002](0002-verified-sequences-only.md) | Only Synapse/OpenRazer sequences, with a lock and read-back | Accepted (guided test retired by 0008) |
| [0003](0003-control-thx-through-its-settings.md) | THX is controlled through its settings, without shipping its driver | Accepted (road chosen in 0004) |
| [0004](0004-thx-over-com.md) | THX settings are changed through its service's COM interface | Accepted (extended by 0005) |
| [0005](0005-thx-over-zeromq.md) | What COM can't change in THX goes over ZeroMQ, with our own client | Accepted |
| [0006](0006-eq-like-synapse.md) | The headset preset also picks THX's preset and curve | Accepted |
| [0007](0007-mic-enhancements-in-the-profile.md) | Mic enhancements live in the profile and rzr sends them to THX again | Accepted |
| [0008](0008-read-only-diagnostics-for-other-models.md) | Models that aren't supported only get read-only diagnostics | Accepted |
| [0009](0009-thx-installer-in-a-separate-repository.md) | THX's installers are offered from a separate repository | Accepted |

## Template

```markdown
# 000X. Title stated as the decision

- Status: Proposed | Accepted | Superseded by 000Y
- Date: YYYY-MM-DD

## Context
What problem there was, and the constraints.

## Decision
What was decided, in one or two clear sentences.

## Alternatives
What else was considered and why not.

## Consequences
What's gained, what's lost and what needs care from now on.
```
