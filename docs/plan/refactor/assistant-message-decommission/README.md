# AssistantMessage Decommission Rollout Checklist

## Scope
- Producer side: stop relying on `AssistantMessage` in normal turn paths.
- Consumer side: use `response_completed` as authoritative final assistant text.
- Compatibility side: keep `assistant_message` parse-tolerant during migration window.

## Cutover Conditions
- Core placeholder ops emit `ResponseStarted + ResponseCompleted` with `Synthetic` source.
- Gateway state reducer treats `response_completed` as authoritative finalization signal.
- Web reducer/hook does not finalize turn from `assistant_message` alone.
- TUI keeps `AssistantMessage` as legacy fallback only and remains `ResponseCompleted`-first.
- Protocol docs/schema mark `assistant_message` as `deprecated` compatibility event.

## Required Validation
- `cargo test -p openjax-core --test m23_assistant_message_decommission_guardrails -- --nocapture`
- `cargo test -p openjax-gateway --test m1_assistant_message_compat_only -- --nocapture`
- `cargo test -p openjax-gateway normal_turn_stream_has_no_assistant_message_event -- --nocapture`
- `cd ui/web && pnpm test -- assistant_message`
- `cd ui/web && pnpm test -- useChatApp.assistant-message-compat.test.ts`

## Observability
- Monitor stream envelopes for unexpected `assistant_message` in live submit path.
- Track client-side turn completion source (should converge to `response_completed`).
- Keep replay/timeline parsing tolerant for legacy history during transition.

## Rollback Conditions
- If clients fail to render completed assistant text after migration.
- If gateway replay/timeline parsing breaks on historical `assistant_message`.
- If TUI or web completion rate drops due to event ordering mismatch.

Rollback action:
- Re-enable compatibility mapping at the affected consumer edge while keeping protocol docs unchanged.
