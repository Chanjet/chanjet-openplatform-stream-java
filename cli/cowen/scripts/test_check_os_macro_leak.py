import unittest
import tempfile
import os
import shutil
import sys

# Ensure scripts directory is on the path to import check_os_macro_leak
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from check_os_macro_leak import is_line_violating, scan_file, check_os_macro_leak

class TestOSMacroLeakCheck(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp()

    def tearDown(self):
        shutil.rmtree(self.test_dir)

    def test_line_violating_detection(self):
        # Violating cases
        self.assertTrue(is_line_violating('#[cfg(target_os = "windows")]'))
        self.assertTrue(is_line_violating('#[cfg(unix)]'))
        self.assertTrue(is_line_violating('if cfg!(target_os = "macos") {'))
        self.assertTrue(is_line_violating('#[cfg_attr(windows, windows_subsystem = "windows")]'))
        self.assertTrue(is_line_violating('#[cfg(target_family = "unix")]'))

        # Non-violating cases
        self.assertFalse(is_line_violating('let config = cfg.get_windows();'))
        self.assertFalse(is_line_violating('// #[cfg(unix)] - this is comment'))
        self.assertFalse(is_line_violating('#[cfg(target_os = "windows")] // os-macro-allowed'))
        self.assertFalse(is_line_violating('/* #[cfg(unix)] */'))
        self.assertFalse(is_line_violating('let path = "cfg(unix)";'))

    def test_file_scanning(self):
        file_path = os.path.join(self.test_dir, 'sample.rs')
        content = """
        fn main() {
            #[cfg(unix)]
            println!("unix");
            
            // #[cfg(windows)]
            println!("hello");
            
            #[cfg(target_os = "macos")] // os-macro-allowed
            println!("mac");
            
            #[cfg(test)]
            mod tests {
                #[cfg(unix)]
                fn t() {}
            }
        }
        """
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)

        violations = scan_file(file_path)
        # Only line 3 (#[cfg(unix)]) is violating
        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0][0], 3)

    def test_directory_scanning(self):
        crates_dir = os.path.join(self.test_dir, 'crates')
        os.makedirs(crates_dir)
        
        # 1. Violating file in regular crate
        bad_dir = os.path.join(crates_dir, 'bad-crate', 'src')
        os.makedirs(bad_dir)
        bad_file = os.path.join(bad_dir, 'lib.rs')
        with open(bad_file, 'w', encoding='utf-8') as f:
            f.write('#[cfg(unix)]\nfn foo() {}')
            
        # 2. Allowed file in cowen-sys
        sys_dir = os.path.join(crates_dir, 'cowen-sys', 'src')
        os.makedirs(sys_dir)
        sys_file = os.path.join(sys_dir, 'lib.rs')
        with open(sys_file, 'w', encoding='utf-8') as f:
            f.write('#[cfg(unix)]\nfn foo() {}')
            
        # 3. Allowed build.rs
        build_file = os.path.join(crates_dir, 'bad-crate', 'build.rs')
        with open(build_file, 'w', encoding='utf-8') as f:
            f.write('#[cfg(unix)]\nfn foo() {}')

        has_leak = check_os_macro_leak(crates_dir)
        self.assertTrue(has_leak)

if __name__ == '__main__':
    unittest.main()
