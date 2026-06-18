import os
import re
import sys

# Regex pattern matching the conditional compilation macros
CFG_PATTERNS = [
    re.compile(r'\bcfg\s*\('),
    re.compile(r'\bcfg_attr\s*\('),
    re.compile(r'\bcfg!\s*\('),
]

# Regex pattern matching OS keywords
OS_KEYS = [
    re.compile(r'\btarget_os\b'),
    re.compile(r'\btarget_family\b'),
    re.compile(r'\bunix\b'),
    re.compile(r'\bwindows\b'),
]

def is_line_violating(line: str) -> bool:
    # 1. Strip comments
    clean_line = line.split('//')[0]
    if '/*' in clean_line:
        clean_line = clean_line.split('/*')[0]
    
    # 2. Check for manual override comment
    if 'os-macro-allowed' in line:
        return False
        
    # 3. Strip string and char literals
    clean_line = re.sub(r'"[^"\\]*(?:\\.[^"\\]*)*"', '', clean_line)
    clean_line = re.sub(r"'[^'\\]*(?:\\.[^'\\]*)*'", '', clean_line)
        
    # 4. Check for cfg structures AND OS keywords
    has_cfg = any(pat.search(clean_line) for pat in CFG_PATTERNS)
    has_os_key = any(pat.search(clean_line) for pat in OS_KEYS)
    
    return has_cfg and has_os_key

def scan_file(path: str) -> list:
    """Returns a list of violating lines as tuples: (line_number, line_content)"""
    violations = []
    try:
        with open(path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
    except Exception as e:
        print(f"Warning: Failed to read {path}: {e}")
        return violations

    in_test_mod = False
    for idx, line in enumerate(lines):
        # Detect if we entered a test module
        if 'mod tests' in line or '#[cfg(test)]' in line:
            in_test_mod = True
            
        if in_test_mod:
            continue
            
        if is_line_violating(line):
            violations.append((idx + 1, line.strip()))
            
    return violations

def check_os_macro_leak(base_dir: str = 'crates') -> bool:
    """Scans the directory for leaks. Returns True if any leaks are found, otherwise False."""
    has_violation = False
    
    for root, _, files in os.walk(base_dir):
        # Exclude cowen-sys and tests folders
        if 'cowen-sys' in root or 'tests' in root:
            continue
            
        for file in files:
            # Exclude build scripts (build.rs)
            if file == 'build.rs':
                continue
            if file.endswith('.rs'):
                path = os.path.join(root, file)
                violations = scan_file(path)
                if violations:
                    has_violation = True
                    for num, content in violations:
                        print(f"❌ OS Macro Leak detected at {path}:{num}: {content}")
                        
    return has_violation

if __name__ == '__main__':
    # Run from the directory of this script or project root
    search_dir = 'crates'
    if not os.path.exists(search_dir) and os.path.exists('cli/cowen/crates'):
        search_dir = 'cli/cowen/crates'
        
    if check_os_macro_leak(search_dir):
        sys.exit(1)
    else:
        print("✅ OS Macro Leak check passed.")
        sys.exit(0)
