"""Linux desktop registration checks; no application build required."""
import contextlib
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import install_linux_desktop as desktop


class LinuxDesktopTests(unittest.TestCase):
    def test_thumbnailers_prefer_system_decoders_and_keep_a_fallback(self):
        with tempfile.TemporaryDirectory() as temporary:
            system_bin = Path(temporary)
            executable = Path('/home/user/BenchUp "test".AppImage')
            fallback = f"[Thumbnailer Entry]\nExec={desktop.desktop_exec(executable)} --thumbnail %i %o\nMimeType=application/x-benchup;\n"
            self.assertEqual(desktop.thumbnailer_entry(executable, system_bin), fallback)
            gdk = system_bin / "gdk-pixbuf-thumbnailer"
            gdk.touch(mode=0o644)
            self.assertEqual(desktop.thumbnailer_entry(executable, system_bin), fallback)
            gdk.chmod(0o755)
            self.assertEqual(desktop.thumbnailer_entry(executable, system_bin),
                             f"[Thumbnailer Entry]\nTryExec={gdk}\nExec={gdk} -s %s %u %o\nMimeType=application/x-benchup;\n")
            glycin = system_bin / "glycin-thumbnailer"
            glycin.touch(mode=0o755)
            self.assertEqual(desktop.thumbnailer_entry(executable, system_bin),
                             f"[Thumbnailer Entry]\nTryExec={glycin}\nExec={glycin} --input %u --output %o --size %s\nMimeType=application/x-benchup;\n")

    def test_installer_upgrades_existing_thumbnail_registration(self):
        with tempfile.TemporaryDirectory() as temporary:
            data = Path(temporary) / "share"
            executable = Path(temporary) / "BenchUp.AppImage"
            executable.touch(mode=0o755)
            target = data / "thumbnailers/benchup.thumbnailer"
            target.parent.mkdir(parents=True)
            target.write_text("old registration")
            expected = desktop.thumbnailer_entry(executable)
            with patch.dict(os.environ, {"XDG_DATA_HOME": str(data)}), \
                 patch.object(desktop.sys, "argv", ["install_linux_desktop.py", str(executable)]), \
                 patch.object(desktop.sys, "platform", "linux"), \
                 patch.object(desktop.shutil, "which", return_value=None), \
                 contextlib.redirect_stdout(io.StringIO()):
                desktop.main()
                self.assertEqual(target.read_text(), expected)
                # Reinstalling is safe and retains the decoder registration.
                desktop.main()
                self.assertEqual(target.read_text(), expected)
            self.assertIn(desktop.desktop_exec(executable),
                          (data / f"applications/{desktop.APP_ID}.desktop").read_text())


if __name__ == "__main__":
    unittest.main()
