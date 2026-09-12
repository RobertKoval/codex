# Cross-Provider Native Subagents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow a native Codex subagent role to select any user-configured model provider (OpenRouter, vLLM, Ollama, MiniMax, Z.ai, or other Responses-compatible providers) without weakening the security boundary introduced by bounded role overrides.

**Architecture:** Cross-provider delegation stays disabled by default. Only user-owned configuration may grant provider IDs through `subagent_model_provider_allowlist`; a user-owned role may then select one of those already-configured providers, while project/session/role layers cannot redefine provider endpoints or credentials. Provider switches rebuild provider-specific routing/auth/model-manager state, and MultiAgent V2 uses a plaintext boundary for external providers while preserving the existing encrypted OpenAI-to-OpenAI path.

**Tech Stack:** Rust, Tokio, Codex config-layer provenance, Responses API, MultiAgent V1/V2, GitHub Actions.

**Spec:** openai/codex#40858 plus the security constraints documented in this plan.

## Global Constraints

- Cross-provider delegation is opt-in and denied by default.
- The allowlist may be granted only by user-owned config or the selected user profile.
- Project config, session flags, and role files cannot extend the allowlist.
- Roles may reference only providers already present in the parent effective `model_providers` map.
- Roles cannot replace provider URL, credentials, auth commands, or provider definitions.
- A provider change must update both provider ID and full provider info and must refresh provider-specific model catalog/manager state.
- OpenAI account auth, attestation/access-program metadata, and encrypted provider-specific payloads must not leak to an external provider.
- Cross-provider children start fresh; explicit partial/full history forks are rejected.
- OpenAI-to-OpenAI MultiAgent V2 retains native encrypted `agent_message`; external destinations receive one ordinary plaintext user task message.
- Encrypted `agent_message` content reaching an external provider fails closed.
- Nested OpenAI→external, external→OpenAI, and external→external delegation must be supported when authorized.
- Cold resume must revalidate persisted provider routing against the current user allowlist and provider definition.

---

### Task 1: Reproduce #40858 on current main

**Files:**
- Create: `codex-rs/core/src/agent/role_provider_tests.rs`
- Modify: `codex-rs/core/src/agent/mod.rs`

**Interfaces:**
- Consumes: existing `apply_role_to_config`, `ConfigBuilder`, `ConfigLayerStack::with_user_config`, `Config::model_providers`.
- Produces: a regression test proving that an explicitly user-authorized role provider must replace `model_provider_id` and `model_provider`.

- [ ] Add a test that configures provider `custom`, adds user TOML `subagent_model_provider_allowlist = ["custom"]`, writes a role with `model_provider = "custom"`, applies the role, and asserts the effective provider changed.
- [ ] Run the focused test in CI and confirm it fails because current bounded role projection discards `model_provider`.
- [ ] Commit the RED test separately.

### Task 2: Add the user-owned provider trust boundary

**Files:**
- Modify: `codex-rs/config/src/config_toml.rs`
- Modify: `codex-rs/config/src/loader/mod.rs`
- Modify: `codex-rs/core/config.schema.json`
- Create: `codex-rs/core/src/config/subagent_model_provider.rs`
- Create: `codex-rs/core/src/config/subagent_model_provider_tests.rs`
- Modify: `codex-rs/core/src/config/mod.rs`
- Modify: `codex-rs/core/src/config/config_loader_tests.rs`
- Modify: `codex-rs/core/src/config/config_tests.rs`

**Interfaces:**
- Produces: `SubagentModelProviderBaseline` and a config helper that resolves only user-owned allowlist entries.
- Security property: lower-trust layers cannot grant provider authority.

- [ ] Add `subagent_model_provider_allowlist: Option<Vec<String>>` to public TOML/schema.
- [ ] Add it to the project-local denylist.
- [ ] Resolve the effective allowlist from user/profile provenance only.
- [ ] Add tests that project/session/role layers cannot extend it and same-provider routing remains allowed without opt-in.
- [ ] Add tests rejecting unknown provider IDs and provider-definition substitution.

### Task 3: Bind provider routing to trusted role provenance

**Files:**
- Modify: `codex-rs/agent-roles/src/agent_role_config.rs`
- Modify: `codex-rs/agent-roles/src/loader.rs`
- Modify: `codex-rs/core/src/agent/role.rs`
- Modify: `codex-rs/core/src/agent/role_tests.rs`

**Interfaces:**
- `AgentRoleConfig` carries a provenance-bound `model_provider` and optional `model_catalog_json` snapshot only for user-owned role files.
- `apply_role_to_config` validates that the snapshot still matches the role file and resolves provider info through `SubagentModelProviderBaseline`.

- [ ] Parse role routing fields separately from bounded capability overrides.
- [ ] Preserve them only for user-owned role definitions.
- [ ] On role application, fail closed if role routing changed since config load.
- [ ] Switch `model_provider_id` and full `model_provider` only after allowlist/provider-map validation.
- [ ] Reset provider-specific model catalog state on provider change.
- [ ] Keep all existing bounded capability restrictions intact.

### Task 4: Isolate child provider/auth/model state and resume behavior

**Files:**
- Create: `codex-rs/core/src/agent/control/model_provider.rs`
- Modify: `codex-rs/core/src/agent/control.rs`
- Modify: `codex-rs/core/src/agent/control/spawn.rs`
- Modify: `codex-rs/core/src/thread_manager.rs`
- Modify: `codex-rs/model-provider/src/auth.rs`
- Modify: `codex-rs/model-provider/src/provider.rs`
- Modify tests in corresponding `*_tests.rs` files and `codex-rs/core/tests/suite/multi_agent_resume.rs`.

**Interfaces:**
- Spawn resolves role provider before validating the selected model.
- Child thread gets provider-scoped auth and a provider-specific model manager/catalog.
- Resume revalidates the stored provider against the current baseline.

- [ ] Add provider baseline resolution for active and stored parent threads.
- [ ] Reject explicit cross-provider full/partial history forks; convert the implicit default fork to fresh only when provider changes.
- [ ] Ensure nested child creation can switch among authorized providers.
- [ ] Drop ambient ChatGPT auth for non-first-party providers unless that provider explicitly requires configured auth.
- [ ] Persist and safely restore the effective provider ID/config.

### Task 5: Make MultiAgent V2 provider-aware

**Files:**
- Modify: `codex-rs/core/src/client.rs`
- Modify: `codex-rs/core/src/client_common.rs`
- Modify: `codex-rs/core/src/stream_events_utils.rs`
- Modify: `codex-rs/core/src/tools/context.rs`
- Modify: `codex-rs/core/src/tools/spec_plan.rs`
- Modify: `codex-rs/core/src/tools/handlers/multi_agent_message.rs`
- Modify: `codex-rs/core/src/tools/handlers/multi_agents_common.rs`
- Modify: `codex-rs/core/src/tools/handlers/multi_agents_spec.rs`
- Modify: `codex-rs/core/src/tools/handlers/multi_agents_v2.rs` and its `spawn`, `send_message`, `message_tool`, `followup_task`, and `surface` modules.
- Modify routing/registry/parallel tests.

**Interfaces:**
- OpenAI destination: existing encrypted `agent_message` behavior.
- External destination: plaintext bridge exposed through a separate `external_agents` surface; outgoing request normalization converts canonical plaintext agent messages to ordinary `user` input.

- [ ] Select collaboration surface according to destination provider.
- [ ] Add `external_agents` only when an authorized external role exists.
- [ ] Lower plaintext agent-message content only in the outgoing request copy; keep durable history canonical.
- [ ] Remove OpenAI-only configuration-update/internal metadata at the external request boundary.
- [ ] Reject encrypted agent-message content for external destinations.
- [ ] Preserve native encrypted behavior for OpenAI children and reverse external→OpenAI delegation.

### Task 6: End-to-end verification and branch completion

**Files:**
- Create/adapt: `codex-rs/core/tests/suite/cross_provider_subagents.rs`
- Modify: `codex-rs/core/tests/suite/mod.rs`
- Adapt affected app-server MultiAgent V2 tests.

**Interfaces:**
- Test matrix covers generic provider IDs; no vLLM/OpenRouter-specific code paths are permitted.

- [ ] V1 OpenAI→external routes model, endpoint, and bearer token correctly.
- [ ] V2 OpenAI→external delivers exactly one user task message and no encrypted agent payload.
- [ ] V2 external→OpenAI uses native OpenAI collaboration path.
- [ ] V2 external→external supports nested provider switching.
- [ ] Unauthorized provider selection, provider substitution, encrypted external payloads, and cross-provider history forks fail closed.
- [ ] Cold resume succeeds only while the provider remains configured and allowlisted.
- [ ] Run targeted Rust tests, `cargo fmt --all -- --check`, relevant clippy/fix checks, and GitHub Actions.
- [ ] Do not claim completion until current-head CI/test evidence is green or remaining unrelated upstream CI failures are explicitly separated from patch-specific failures.
