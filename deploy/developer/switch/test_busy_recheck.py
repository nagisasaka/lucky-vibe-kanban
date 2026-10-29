"""Only initial read-only preflight may wait; production delay is asserted."""
import argparse
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import start_binary as engine
import start_support as support


class BusyRecheckTest(unittest.TestCase):
    def check(self, outcomes):
        with tempfile.TemporaryDirectory(prefix="lvk-busy-test-") as temp:
            stage = Mock()
            with patch.object(engine, "preflight", side_effect=outcomes) as preflight, patch.object(engine.time, "sleep") as sleep:
                try:
                    result = engine.initial_preflight(argparse.Namespace(), Path(temp), {"identity": "fixed"}, stage)
                except support.Refuse as error:
                    result = error
                return result, preflight, sleep, stage, (Path(temp) / "busy-recheck.json").exists()

    def test_idle_is_immediate(self):
        result, check, sleep, stage, record = self.check([{"ok": True}])
        self.assertEqual(result, {"ok": True})
        self.assertEqual(check.call_count, 1)
        sleep.assert_not_called()
        self.assertFalse(record)

    def test_busy_then_idle_uses_production_30_seconds_once(self):
        self.assertEqual(engine.BUSY_RECHECK_SECONDS, 30)
        result, check, sleep, stage, record = self.check([support.Busy("descendants"), {"ok": True}])
        self.assertEqual(result, {"ok": True})
        self.assertEqual(check.call_count, 2)
        sleep.assert_called_once_with(30)
        self.assertTrue(record)
        self.assertEqual([c.args[0] for c in stage.call_args_list], ["busy-wait", "preflight"])

    def test_busy_twice_stops_without_third_check(self):
        result, check, sleep, _, _ = self.check([support.Busy("one"), support.Busy("two")])
        self.assertIsInstance(result, support.Busy)
        self.assertEqual(check.call_count, 2)
        sleep.assert_called_once_with(30)

    def test_nonbusy_never_waits_even_after_a_busy(self):
        result, check, sleep, _, _ = self.check([support.Refuse("hash mismatch")])
        self.assertIsInstance(result, support.Refuse)
        sleep.assert_not_called()
        result, check, sleep, _, _ = self.check([support.Busy("child"), support.Refuse("identity changed")])
        self.assertEqual(str(result), "identity changed")
        self.assertEqual(check.call_count, 2)
        sleep.assert_called_once_with(30)

    def test_physical_children_and_instance_users_are_busy_but_inspection_is_not(self):
        with patch.object(support, "descendants", return_value={123}):
            with self.assertRaises(support.Busy): support.idle(42)
        with patch.object(support, "descendants", return_value=set()), patch.object(support, "pre_stop_writers", side_effect=support.Refuse("Cannot inspect live process")):
            with self.assertRaises(support.Refuse) as e: support.idle(42)
            self.assertNotIsInstance(e.exception, support.Busy)


if __name__ == "__main__": unittest.main()
