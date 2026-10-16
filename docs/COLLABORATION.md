# Native direct collaboration — stage 13

The normal build now includes native LAN/VPN Share/Join, using [Yrs 0.28.0](https://docs.rs/yrs/0.28.0/yrs/) rich-text CRDTs and direct encrypted Iroh connections. Network activity starts only when the user hosts or joins. Source, UI-frame and actual encrypted loopback checks pass; final packaged/native verification is recorded in PROGRESS.md. This is not a claim of independent security review or full Google Docs parity.

## Using the current implementation

Open Share or File → Share or join. The host chooses a display name and local LAN/VPN address, starts sharing and privately sends a copied edit/view invitation. Guests paste it into Join a session; the homepage also has Join session. The host must remain open and reachable. No account, rented backend, public relay, Google OAuth or email delivery is configured. Names are participant-supplied, not verified identities.

The invitation pins the host key and contains a random 256-bit bearer grant. The host enforces edit/view access and one-hour expiry. Ending sharing or closing the host document ends access; revocation rejects subsequent requests, but cannot erase an already received copy. The native mode selector cannot elevate a viewer's role, and the engine rejects viewer edits even if the UI state is changed.

## Current behavior and bounds

- Text and per-property format marks use Yrs sequences; maps/arrays preserve shared object identities for editable paragraphs, table cells, tabs, images and annotation data. Local changes enter through the engine. Remote updates merge before projecting a validated native document. Session undo uses Yrs local origins, preserving other actors' changes rather than restoring old whole-document snapshots.
- Tab navigation, caret, selection, viewport, local document ID and starred state remain participant-local. Joining creates a fresh local document instead of overwriting an existing host-ID file. Other open-file sessions remain independent. Starting/leaving a collaboration session starts a new undo history.
- Text annotations and remote selections use stable cursor endpoints, with explicit UTF-16/scalar conversion and grapheme-safe clamping. Original images remain immutable shared asset buffers, cached as SharedVec; typing does not retransmit source images. Schema 9 retains chosen comment display names; reply strings identify the session name. Reactions synchronize as the existing toggle model, without per-person counts.
- State-vector differences synchronize incremental changes and deletions. Connection failures retry while retaining local edits. The actual QUIC test interrupts connections, edits on both sides and verifies convergence after recovery. Duplicate/reordered updates and three-replica convergence are tested separately.
- Limits are eight active guests, 32 MiB encoded session state, bounded structure/text and six-second requests. Binary updates use base64 inside a bounded wire envelope. Speculative merged documents are validated before accepted network state changes. Rejected updates pause synchronization and retain the sender's draft locally.
- Native saves/exports retain merged content and comment names; autosave obeys the existing preference. CRDT binary save/load is tested, but native files do not embed the session log yet. Connections, invitations and session undo do not resume after restarting the app.

Concurrent structural changes do not yet have complete Docs intent semantics. A concurrent paragraph split/join or move can leave edits in the surviving original object; incompatible table geometry or competing tab deletion can be rejected for manual review. Remaining work includes semantic structural merging, session log/restart recovery, streamed larger assets, individual peer admission/review/revocation UI, per-person reactions, relay connectivity, verified identity and independent adversarial/security testing. Do not claim production-audited security.

## Historical investigation and pre-implementation audit

The research and audit below describe earlier stages. The legacy v1 snapshot/proposal protocol in `src/collaboration.rs` is retained for regressions and is not used by the UI. Stage 13 uses `collaboration/live.rs`, `collaboration/replica.rs` and `engine/shared.rs`; the current behavior above supersedes the earlier missing-feature assessments.

## What FigCraft actually does

Inspected its [collaboration protocol](https://github.com/storytold/figcraft/blob/main/docs/collab-protocol.md), [network entry point](https://github.com/storytold/figcraft/blob/main/crates/collab-net/src/lib.rs) and [embedded host](https://github.com/storytold/figcraft/blob/main/crates/collab-net/src/host.rs). It embeds a WebSocket session host for LAN sharing and uses a separately hosted server online. Its authority orders property updates; conflicts use last-writer-wins in server order. That is not an adequate text merge strategy for concurrent rich-text edits. Its [privacy statement](https://github.com/storytold/figcraft#privacy-and-online-sharing) says LAN sharing is unencrypted and online servers can read stored files. We are not copying this code or security model.

## No hosted backend versus no servers anywhere

A participant can host a session inside the native app; this needs no rented server or accounts, but that participant's device must be reachable and remain awake. Direct LAN/VPN connections are the first scope. Arbitrary internet reachability through NAT/firewalls is a different requirement. Iroh can use relays that forward encrypted packets, but those are still third-party servers. Do not call relay-based connectivity literally serverless. No public relay, DNS address lookup or automatic port mapping is enabled by AlphaDocs's direct-peer prototype.

## Protocol foundation

Use maintained [Iroh 1.3](https://docs.rs/iroh/1.3.0/iroh/) for QUIC/TLS, peer-key authentication and encryption. Use its [empty endpoint builder](https://docs.rs/iroh/1.3.0/iroh/endpoint/struct.Builder.html#method.empty), explicitly restricted IP transports and no relay/address lookup services. Bind only a caller-selected address. Tests bind loopback, not a public interface.

The host controls read/edit grants; a joining peer cannot select its own role. Invitations pin the host public key and contain a random 256-bit capability, with host-enforced expiry. Bearer invitations can be forwarded and must be treated as secrets; their contents must not be logged or sent automatically. Capabilities are compared in constant time. Authenticate before releasing document data, bound message sizes, impose read/connection deadlines, reject invalid documents, and close endpoints to end a session. Receiving data does not grant permission to overwrite local files or open embedded links.

The prototype supports versioned snapshots and proposals with a base revision. A stale proposal is rejected with a conflict response; it is never silently used as a replacement. This is conflict detection, not collaborative merging. The caller must preserve its rejected local draft. No UI is wired to discard or overwrite it.

## Historical planned stages

1. Structured CRDT/OT adapter: concurrent Unicode text inserts/deletes, style marks, block/table ordering, images, content tabs and anchored comments. Use [Automerge's Rust sync protocol](https://docs.rs/automerge/latest/automerge/sync/index.html) or a suitable rich-text CRDT; prove convergence under concurrent edits, reconnects and out-of-order delivery. Replacing entire paragraph strings with last-writer-wins is unacceptable.
2. Per-user undo that preserves other people's changes; durable local recovery, bounded asset streaming and conflict recovery.
3. Native share/join dialog, peer verification, explicit grants/revocation, presence and connection status. No fake Share button before these work.
4. Optional explicitly chosen encrypted relay connectivity and independent security review/fuzzing/resource-abuse tests before claiming production safety.

## Historical five-area audit — before stage 13 implementation

The user's latest research separates concurrent editing, rendering, pagination, rich-object anchoring and remote selections. These are retained requirements, not evidence that the linked tutorial clones implement all five. AlphaDocs remains a native Rust editor; a React/contenteditable or Firebase rewrite is outside its architecture.

| Area | Current source behavior | Remaining work |
| --- | --- | --- |
| Concurrent edits | `src/collaboration.rs` accepts a whole-document proposal only against the current revision and returns a conflict for a stale base. | Operation-level merging, local undo that preserves remote work, durable recovery, reconnect and native integration. No OT/CRDT is implemented. |
| Rendering | `src/layout.rs` shapes and measures native text; screen and PDF share logical document geometry. Fonts are real installed/bundled faces. | Cross-platform runtime evidence, broader shaping coverage and consistent font availability. A custom renderer alone does not guarantee identical page breaks across devices. |
| Pagination | Measured body/table layout, columns, repeating text headers/footers, paragraph keep rules and ordinary-paragraph widow control exist. | Footnotes, complex sections and full keep/widow behavior around floating objects. Do not describe existing ordinary-paragraph checks as complete Word/Docs pagination parity. |
| Rich anchoring | Images retain IDs, flow/offsets and object annotations; table/paragraph content is editable; local text annotations follow actual edits and paragraph splits/joins. | Stable replicated block/cell identities and concurrent anchor transforms; true inline image atoms, floating text boxes and nested/merged tables. Indentation/basic list markers are not a complete semantic nested-list model. |
| Remote cursors | Local selection and comment anchors are handled by the engine. | Peer presence, stable remote range endpoints, insert affinity, deletion fallback and native remote-cursor painting. Local offset rebasing does not establish distributed correctness. |

### Next implementation gate: merge semantics before sharing UI

Evaluate a maintained Rust CRDT through an isolated, non-shipping adapter first. [Automerge's Rust documentation](https://docs.rs/automerge/latest/automerge/) describes text objects, incremental patches, synchronization and stable cursors. It is a candidate, not an adopted dependency. A successful library-level merge is not evidence that AlphaDocs's formatting, undo or annotations merge correctly.

1. Give replicated tabs, blocks and table cells persistent identities. Store text as editable sequences and formatting as ranges/marks; never replicate a paragraph as a replaceable scalar string. Retain conflict information for incompatible property changes and define delete-versus-edit outcomes explicitly. Image bytes remain immutable shared assets, separate from frequent text/presence updates.
2. Keep the active content tab, selection, viewport and tool state local to each participant. `Document.blocks` currently projects the active tab, with inactive storage in tab entries; serializing that projection as peer state would confuse navigation with content edits. Materialize each participant's chosen tab from the replicated content instead.
3. Map CRDT positions explicitly to AlphaDocs's Unicode-scalar offsets and grapheme-safe editing boundaries. Automerge documents UTF-8 indexing by default on native Rust; these are not interchangeable with character or grapheme indices. Preserve endpoint affinity at insertions and specify how anchors resolve when their referenced text, paragraph, cell or tab is deleted.
4. Integrate remote patches through engine transactions and validation, retaining SharedVec copy-on-write media/text storage. Collaborative undo must reverse the participant's intent without restoring an old whole-document snapshot over another participant's edits. Existing offline undo remains intact until this works.
5. Synchronize durable document changes separately from ephemeral presence. Presence carries stable cursor/range references and a tab identity, expires on disconnect, and does not enter document undo, exports or saved comments. Show offline/reconnecting status; immediate local typing is feasible, but zero network delay is not a promise.

Before connecting the adapter to Share/Join, require deterministic two- and three-replica tests for simultaneous same-position insertion; overlapping insertion/deletion and formatting; combining marks, emoji/ZWJ sequences and mixed scripts; paragraph splits/joins; block/tab/table-cell edits and deletion; moved-image annotations; and local undo after remote edits. Exchange changes in varied orders with duplicates and disconnection/reconnection, respecting any library sync protocol's transport-order requirements. Verify converged structured content, bounded valid selections/anchors, retained assets and save/reload continuation. Do not mistake convergence alone for preservation of editing intent.

The next separate gate is transport integration: per-peer synchronization state, bounded messages/assets, permissions enforced before applying edits, durable local recovery and reconnect tests. Stable cursor references do not authenticate a peer, and a CRDT does not supply access control. Default builds and files remain unchanged until these gates are met; sharing stays unavailable in the shipping interface.

## Google accounts

[Google's desktop OAuth flow](https://developers.google.com/identity/protocols/oauth2/native-app) supports an external system browser, authorization-code flow with PKCE and a temporary loopback callback on desktop. A hosted AlphaDocs login server is not required for that flow, but a registered Google Cloud project/OAuth desktop client and configured consent screen are required. A distributable desktop app cannot keep a client secret confidential. Validate state/nonce, token issuer/audience/expiry and protect refresh tokens in the OS credential store.

Google sign-in supplies identity, not document transport, presence, permissions or concurrent merging. Google Drive could provide hosted storage and sharing, but then Google is the server and API scopes/verification requirements apply. Identity-based email grants also need a verifiable binding between the Google identity and the peer key. Do not assume a display name or email field proves identity. No accounts were registered, OAuth credentials created, user files uploaded or emails sent in this work.

## Historical prototype verification

Two actual encrypted QUIC loopback tests pass under `cargo test --all-features`: host-enforced view/edit roles, rejected stale proposals with the caller retaining its draft, invalid capabilities, expiry/revocation, redacted invite debug text, and a wrong pinned host key unable to read the document. Minimal endpoint preset supplies the Rustls crypto provider; IP transports, discovery/reporting and port mapping remain explicitly restricted. No public peer session, real network NAT/reconnect test, concurrent editing UI or independent security audit has been performed.


## Nearby invitations (0.1.2)

Share → Make this device visible enables a temporary, opt-in discovery service. Both devices must enable it on the same private IPv4 LAN and run a compatible AlphaDocs build. A TTL-1 IPv4 multicast beacon (239.255.77.77:44764) advertises only a chosen name, platform, ephemeral endpoint public key and port. It carries no document title, text or invitation capability. Public-source beacons, malformed/oversized messages and excess device entries are rejected; devices expire from the list after eight seconds. Firewalls and some Wi-Fi/VPN configurations block multicast, so manual invitations remain available.

Invite sends the private live-session invitation/title over a separate bounded encrypted Iroh protocol pinned to the selected device's public key. The receiver must choose Join; receipt alone does not load a document or connect to the host. At most four offers can wait, one per sender; pending offers expire after 60 seconds. Nearby is stopped explicitly or on app exit, and is never restored automatically. Display/device names are not authenticated accounts; compare short IDs on the intended computers. Opt-in visibility is not a security audit or an internet sharing/relay service. It does not implement AirDrop or standalone file transfer.

The Working together strip shows each connected person's current content tab (not a typing claim), plus a short ID for duplicate names. Caret/selection badges use the same stable colours. Peer names/tab titles are read from the shared room; local navigation remains local.
