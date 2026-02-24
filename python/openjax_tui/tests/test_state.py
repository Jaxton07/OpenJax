from __future__ import annotations

import unittest

from openjax_tui.state import (
    AnimationLifecycle,
    AppState,
    ApprovalRecord,
    LiveViewportOwnership,
    ViewMode,
)


class StateModuleTest(unittest.TestCase):
    def test_appstate_defaults(self) -> None:
        state = AppState()
        self.assertTrue(state.running)
        self.assertEqual(state.input_backend, "basic")
        self.assertEqual(state.turn_phase, "idle")
        self.assertEqual(len(state.pending_approvals), 0)
        self.assertEqual(state.view_mode, ViewMode.LIVE_VIEWPORT)
        self.assertEqual(state.animation_lifecycle, AnimationLifecycle.IDLE)
        self.assertIsNone(state.live_viewport_owner_turn_id)
        self.assertEqual(state.live_viewport_turn_ownership, {})

    def test_approval_record_defaults(self) -> None:
        record = ApprovalRecord(turn_id="t1", target="tool", reason="r")
        self.assertEqual(record.status, "pending")

    def test_normalize_view_mode_defaults_to_live_viewport(self) -> None:
        self.assertEqual(AppState.normalize_view_mode(None), ViewMode.LIVE_VIEWPORT)
        self.assertEqual(AppState.normalize_view_mode(""), ViewMode.SESSION)
        self.assertEqual(AppState.normalize_view_mode("unknown"), ViewMode.SESSION)

    def test_normalize_view_mode_accepts_valid_values(self) -> None:
        self.assertEqual(AppState.normalize_view_mode("session"), ViewMode.SESSION)
        self.assertEqual(AppState.normalize_view_mode("live"), ViewMode.LIVE_VIEWPORT)
        self.assertEqual(
            AppState.normalize_view_mode("live_viewport"),
            ViewMode.LIVE_VIEWPORT,
        )
        self.assertEqual(
            AppState.normalize_view_mode(ViewMode.LIVE_VIEWPORT),
            ViewMode.LIVE_VIEWPORT,
        )

    def test_set_view_mode_uses_deterministic_normalization(self) -> None:
        state = AppState()
        self.assertEqual(state.set_view_mode("live"), ViewMode.LIVE_VIEWPORT)
        self.assertEqual(state.view_mode, ViewMode.LIVE_VIEWPORT)
        self.assertEqual(state.set_view_mode("live_viewport"), ViewMode.LIVE_VIEWPORT)
        self.assertEqual(state.view_mode, ViewMode.LIVE_VIEWPORT)
        self.assertEqual(state.set_view_mode("invalid-mode"), ViewMode.SESSION)
        self.assertEqual(state.view_mode, ViewMode.SESSION)

    def test_live_viewport_ownership_value_contract(self) -> None:
        state = AppState()
        state.live_viewport_owner_turn_id = "turn-1"
        state.live_viewport_turn_ownership["turn-1"] = LiveViewportOwnership.ACTIVE
        state.live_viewport_turn_ownership["turn-2"] = LiveViewportOwnership.RELEASED
        self.assertEqual(state.live_viewport_owner_turn_id, "turn-1")
        self.assertEqual(
            state.live_viewport_turn_ownership["turn-1"],
            LiveViewportOwnership.ACTIVE,
        )
        self.assertEqual(
            state.live_viewport_turn_ownership["turn-2"],
            LiveViewportOwnership.RELEASED,
        )


if __name__ == "__main__":
    _ = unittest.main()
