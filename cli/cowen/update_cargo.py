import os
import re

groups = {
    'cowen-infra': 'core', 'cowen-sys': 'core', 'cowen-common': 'core', 'cowen-plugin': 'core',
    'cowen-config': 'services', 'cowen-monitor': 'services', 'cowen-doctor': 'services',
    'cowen-store': 'services', 'cowen-auth': 'services', 'cowen-search': 'services', 'cowen-ai': 'services',
    'cowen-search-embedding': 'plugins', 'cowen-mcp-plugin': 'plugins',
    'cowen-server': 'app', 'cowen-daemon': 'app', 'cowen-cli': 'app',
    'cowen-signer': 'tools'
}

for root, dirs, files in os.walk('crates'):
    for file in files:
        if file == 'Cargo.toml':
            path = os.path.join(root, file)
            with open(path, 'r') as f:
                content = f.read()

            def repl_internal(m):
                crate_name = m.group(1)
                if crate_name in groups:
                    return f'path = "../../{groups[crate_name]}/{crate_name}"'
                return m.group(0)

            # Replace internal cowen crates path
            content = re.sub(r'path\s*=\s*"../(cowen-[a-zA-Z0-9-]+)"', repl_internal, content)
            
            # Replace sdk path
            content = re.sub(r'path\s*=\s*"../../../../sdk/rust"', 'path = "../../../../../sdk/rust"', content)

            with open(path, 'w') as f:
                f.write(content)
