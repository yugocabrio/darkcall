"""Recovery/public dispatch must not convert invalid metadata into legacy keys."""
import json
import pathlib
import tempfile
import unittest
from native_layout import read_layout, WORKING, SHARED


class LayoutTests(unittest.TestCase):
    def test_absence_only_is_historical_and_known_metadata_dispatches(self):
        with tempfile.TemporaryDirectory() as path:
            directory = pathlib.Path(path)
            self.assertEqual(read_layout(directory), WORKING)
            for layout in (WORKING, SHARED):
                (directory / 'native-layout.json').write_text(json.dumps({'format': 1, 'layout': layout}))
                self.assertEqual(read_layout(directory), layout)

    def test_unknown_versions_types_extra_keys_and_uploaded_vk_fail(self):
        with tempfile.TemporaryDirectory() as path:
            directory = pathlib.Path(path)
            for value in [
                {'format': 2, 'layout': SHARED}, {'format': True, 'layout': SHARED},
                {'format': 1.0, 'layout': SHARED}, {'format': 1, 'layout': 'upload-vk'},
                {'format': 1, 'layout': SHARED, 'vk': 'attacker'}, [], {'format': 1},
            ]:
                (directory / 'native-layout.json').write_text(json.dumps(value))
                with self.assertRaises(RuntimeError):
                    read_layout(directory)

    def test_symlinks_and_oversized_metadata_never_fall_back(self):
        with tempfile.TemporaryDirectory() as path:
            directory = pathlib.Path(path)
            marker = directory / 'native-layout.json'
            marker.symlink_to(directory / 'missing')
            with self.assertRaises(RuntimeError):
                read_layout(directory)
            marker.unlink()
            marker.write_bytes(b' ' * 4097)
            with self.assertRaises(RuntimeError):
                read_layout(directory)


if __name__ == '__main__':
    unittest.main()
