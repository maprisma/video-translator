"""Desktop-entry value and Exec quoting, including hostile-looking home paths."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('desktop_exec', Path(__file__).resolve().parents[1] / 'desktop-exec.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class DesktopExecTests(unittest.TestCase):
    def test_normal_path_and_action_arguments(self):
        self.assertEqual(module.render('Exec=video-trans input "Audio output"\n', '/home/user/.local/bin/video-trans'),
                         'Exec="/home/user/.local/bin/video-trans" input "Audio output"\n')

    def test_both_escaping_layers_and_percent_field_code(self):
        self.assertEqual(module.quote_executable('/tmp/a b%"$`\\bin'),
                         '"/tmp/a b%%\\\\"\\\\$\\\\`\\\\\\\\bin"')
