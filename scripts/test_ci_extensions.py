"""Regression checks for CI host coverage and isolated Apple consumers."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_apple_package as apple
import check_ci_languages as languages


class ExtensionTests(unittest.TestCase):
    def test_missing_language_fails_before_running(self):
        with patch.object(languages.sys, 'platform', 'linux'), patch.object(languages.shutil, 'which', return_value=None):
            with self.assertRaisesRegex(RuntimeError, 'Missing required SDK tools:.*swift.*dotnet'):
                languages.check(lambda *a, **kw: self.fail('Started with missing tools'), Path('.'))

    def test_available_ios_simulator_required(self):
        devices = {'devices': {'com.apple.CoreSimulator.SimRuntime.watchOS-12-0': [
            {'name': 'iPhone wrong platform', 'isAvailable': True, 'udid': 'wrong'}],
            'com.apple.CoreSimulator.SimRuntime.iOS-26-0': [
            {'name': 'iPhone missing', 'isAvailable': False, 'udid': 'missing'},
            {'name': 'iPhone 17', 'isAvailable': True, 'udid': 'available'}]}}
        self.assertEqual(apple.simulator_id(devices), 'available')
        with self.assertRaisesRegex(RuntimeError, 'No available iPhone simulator'):
            apple.simulator_id({'devices': {}})

    def test_apple_consumer_uses_copied_binary_and_bundled_fixtures(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            framework = directory / 'source.xcframework'
            framework.mkdir()
            (framework / 'Info.plist').write_text('fixture')
            package = apple.prepare_consumer(directory, framework)
            self.assertEqual((package / 'CUniLyrics.xcframework/Info.plist').read_text(), 'fixture')
            manifest = (package / 'Package.swift').read_text()
            self.assertIn('.binaryTarget(name: "CUniLyrics"', manifest)
            self.assertNotIn(str(apple.ROOT), manifest)
            for source in (package / 'Tests/UniLyricsTests').glob('*.swift'):
                text = source.read_text()
                self.assertNotIn('root.appendingPathComponent', text)
                self.assertNotIn('#filePath', text)
            self.assertTrue((package / 'Tests/UniLyricsTests/Fixtures/conformance.json').is_file())


if __name__ == '__main__':
    unittest.main()
