import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import store


class ConfigTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self._tmp.name)
        self.config_path = self.dir / "config.json"
        self.notes_path = self.dir / "notes.txt"

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_defaults_when_missing(self) -> None:
        cfg = store.load_config(self.config_path)
        self.assertEqual(cfg.width, 360)
        self.assertEqual(cfg.font_size, 18)
        self.assertEqual(cfg.color, "#ffffff")
        self.assertEqual(cfg.opacity, 1.0)
        self.assertTrue(cfg.outline)
        self.assertIsNone(cfg.x)

    def test_roundtrip(self) -> None:
        cfg = store.Config(x=120, y=340, width=520, font_size=24, color="#ffcc00", opacity=0.7, outline=False)
        store.save_config(cfg, self.config_path)
        loaded = store.load_config(self.config_path)
        self.assertEqual(loaded, cfg)

    def test_corrupt_file_is_backed_up(self) -> None:
        self.config_path.write_text("{not json", encoding="utf-8")
        cfg = store.load_config(self.config_path)
        self.assertEqual(cfg.width, 360)
        self.assertFalse(self.config_path.exists())
        backups = list(self.dir.glob("config.json.corrupt-*"))
        self.assertEqual(len(backups), 1)

    def test_invalid_values_fall_back_to_defaults(self) -> None:
        self.config_path.write_text(
            json.dumps(
                {
                    "x": True,
                    "y": 10,
                    "width": 99999,
                    "font_size": 3,
                    "color": "red",
                    "opacity": 5,
                    "outline": "yes",
                }
            ),
            encoding="utf-8",
        )
        cfg = store.load_config(self.config_path)
        self.assertEqual(cfg.width, 1200)
        self.assertEqual(cfg.font_size, 10)
        self.assertEqual(cfg.color, "#ffffff")
        self.assertEqual(cfg.opacity, 1.0)
        self.assertTrue(cfg.outline)
        self.assertIsNone(cfg.x)
        self.assertIsNone(cfg.y)

    def test_notes_default_created_then_roundtrip(self) -> None:
        text = store.load_notes(self.notes_path)
        self.assertEqual(text, store.DEFAULT_TEXT)
        self.assertTrue(self.notes_path.exists())

        store.save_notes("第一行\n第二行", self.notes_path)
        self.assertEqual(store.load_notes(self.notes_path), "第一行\n第二行")

    def test_atomic_write_leaves_no_temp_files(self) -> None:
        store.save_notes("hello", self.notes_path)
        leftovers = [p.name for p in self.dir.iterdir() if p.name.endswith(".tmp")]
        self.assertEqual(leftovers, [])


@unittest.skipUnless(shutil.which("git"), "git 不可用")
class HistoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self._tmp.name)
        self.notes_path = self.dir / "notes.txt"
        self.history_dir = self.dir / "notes-history"

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def commits(self) -> int:
        result = subprocess.run(
            ["git", "-C", str(self.history_dir), "log", "--oneline"],
            capture_output=True,
            text=True,
        )
        output = result.stdout.strip()
        return len(output.splitlines()) if output else 0

    def record(self, text: str, moment: datetime) -> None:
        store.record_history(
            text,
            notes_path=self.notes_path,
            history_dir=self.history_dir,
            now=moment,
        )

    def test_start_creates_repo_and_initial_commit(self) -> None:
        store.start_history(
            "a\nb",
            history_dir=self.history_dir,
            now=datetime(2026, 1, 2, 3, 4, 5),
        )
        self.assertTrue((self.history_dir / ".git").exists())
        self.assertEqual(self.commits(), 1)

    def test_each_save_creates_commit(self) -> None:
        store.start_history(
            "v1", history_dir=self.history_dir, now=datetime(2026, 1, 2, 8, 0, 0)
        )
        self.record("v2", datetime(2026, 1, 2, 9, 0, 0))
        self.assertEqual(store.load_notes(self.notes_path), "v2")
        self.assertEqual(self.commits(), 2)

    def test_unchanged_save_does_not_commit(self) -> None:
        store.start_history(
            "same", history_dir=self.history_dir, now=datetime(2026, 1, 2, 8, 0, 0)
        )
        self.record("same", datetime(2026, 1, 2, 9, 0, 0))
        self.assertEqual(self.commits(), 1)

    def test_history_increment_shows_added_and_removed(self) -> None:
        store.start_history(
            "任务一\n任务二", history_dir=self.history_dir, now=datetime(2026, 1, 2, 8, 0, 0)
        )
        self.record("任务二\n任务三", datetime(2026, 1, 2, 9, 0, 0))

        report = store.history_increment(
            datetime(2026, 1, 1), datetime(2026, 1, 3), history_dir=self.history_dir
        )
        self.assertIn("+ 任务一", report)
        self.assertIn("+ 任务三", report)
        self.assertIn("- 任务一", report)

    def test_history_increment_filters_by_time(self) -> None:
        store.start_history(
            "初始专有内容", history_dir=self.history_dir, now=datetime(2026, 1, 2, 8, 0, 0)
        )
        self.record("后续新增", datetime(2026, 1, 2, 9, 0, 0))

        report = store.history_increment(
            datetime(2026, 1, 2, 8, 30), datetime(2026, 1, 2, 9, 30), history_dir=self.history_dir
        )
        self.assertIn("+ 后续新增", report)
        self.assertNotIn("+ 初始专有内容", report)
        self.assertIn("- 初始专有内容", report)

    def test_history_bounds(self) -> None:
        store.start_history(
            "v1", history_dir=self.history_dir, now=datetime(2026, 1, 2, 8, 0, 0)
        )
        self.record("v2", datetime(2026, 1, 2, 9, 30, 0))
        first, last = store.history_bounds(history_dir=self.history_dir)
        self.assertEqual(first, datetime(2026, 1, 2, 8, 0, 0))
        self.assertEqual(last, datetime(2026, 1, 2, 9, 30, 0))

    def test_increment_without_history(self) -> None:
        report = store.history_increment(
            datetime(2026, 1, 1), datetime(2026, 1, 2), history_dir=self.history_dir
        )
        self.assertIn("还没有保存历史", report)


if __name__ == "__main__":
    unittest.main()