"""Check native consumer selection without requiring foreign host toolchains."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import check_ci_desktop as ci


class DesktopTests(unittest.TestCase):
    def test_host_library_and_shared_protocol(self):
        for platform, name in [('linux', 'libunilyrics_ffi.so'),
                               ('darwin', 'libunilyrics_ffi.dylib'),
                               ('win32', 'unilyrics_ffi.dll')]:
            with self.subTest(platform=platform), TemporaryDirectory() as temporary:
                calls = []
                with patch.object(ci.sys, 'platform', platform), patch.object(ci, 'check_protocol') as protocol:
                    ci.check(lambda command, **kwargs: calls.append(command), Path(temporary))
                library = ci.ROOT / 'target/debug' / name
                self.assertIn([ci.sys.executable, '-B', 'bindings/python/test_conformance.py', library], calls)
                self.assertEqual(protocol.call_args.args[0][-1], str(library))
                self.assertIn('<clear />', (Path(temporary) / 'NuGet.Config').read_text())

    def test_failure_propagates(self):
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(RuntimeError, 'build failed'):
                ci.check(lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError('build failed')), Path(temporary))


if __name__ == '__main__':
    unittest.main()
