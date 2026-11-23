import json
import sys
import tempfile
import unittest
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


if __name__ == "__main__":
    unittest.main()