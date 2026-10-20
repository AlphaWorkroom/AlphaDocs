# Stage 35 — speech download release blocker

The second v0.1.11 attempt passed Linux and both macOS packages, but Windows again failed while downloading the 41,205,931-byte speech model. Its body-transfer timeout fired exactly 20 seconds after the response began, despite progress arriving. Publication was skipped.

The ureq configuration documentation specifies that `timeout_recv_body` bounds the entire transfer and does not restart on each read. Both pinned runtime and model transfers now use 300 seconds, matching the existing global request bound. Size limits, SHA-256 verification, staging cleanup and cancellation remain unchanged. The correction is version 0.1.12; the existing failed tag is retained.

No computer control was used. The unchanged hyperlink interaction test already passed for right-click first-row targeting, single Ctrl/Cmd navigation, and no navigation for Shift or Ctrl/Cmd+Shift selection, with split and coalesced pointer events. Full platform checks and explicit network speech setup run in release CI; local offline speech checks are recorded after completion.

Local results: all 10 speech-module tests passed; rustfmt, whitespace checks and release-version parsing passed. The explicit real download and four-platform package checks remain pending in CI. No cross-platform runtime success is claimed from these local checks.
