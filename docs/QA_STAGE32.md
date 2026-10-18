# Stage 32 — standard release numbering

18 October 2026. Source `4559bc7e474d9526092c02ebfb4949910f150b25`, release v0.1.9.

This changes only the package version, lockfile version and current release documentation. Application code is unchanged from the [verified homepage patch](QA_STAGE31.md). Its 287 passing tests, three final homepage regressions and native wrapping/scroll/open checks are reused. No redundant editor tests were rerun.

Locked Cargo metadata and installer/bundle version mapping verify 0.1.9; whitespace checks pass. The three-part release number is readable by existing update clients. Version 0.1.6/0.1.7 Windows copies outside the normal installation folder still need the previously documented one-time Setup installation because their own code contains that older path restriction.

[Native builds and installer verification](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39120039714) are pending. Existing release tags/assets are retained. The superseded 0.1.8.1 build was checked once; status: completed.
