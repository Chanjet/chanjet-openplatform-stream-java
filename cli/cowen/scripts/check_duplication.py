import json
import sys

def check_duplication():
    try:
        with open('target/jscpd/jscpd-report.json') as f:
            data = json.load(f)
    except Exception as e:
        print('Failed to parse jscpd report:', e)
        sys.exit(1)

    duplicates = data.get('duplicates', [])
    failed = False

    # 1. 相同文件内不允许出现超过 5 行的重复代码
    same_file_clones = []
    for clone in duplicates:
        f1 = clone.get('firstFile', {}).get('name')
        f2 = clone.get('secondFile', {}).get('name')
        lines = clone.get('lines', 0)
        if f1 == f2 and lines > 5:
            same_file_clones.append(
                f"{f1} lines {clone['firstFile']['start']}-{clone['firstFile']['end']} and {clone['secondFile']['start']}-{clone['secondFile']['end']} ({lines} lines)"
            )

    if same_file_clones:
        print('❌ Code duplication check failed! Same-file duplication of >5 lines is not allowed.')
        for c in same_file_clones:
            print('  -', c)
        failed = True
    else:
        print('✅ No same-file duplications of >5 lines found.')

    # 2. 任何文件之间不允许出现超过 15 行及以上的重复代码
    long_clones = []
    for clone in duplicates:
        f1 = clone.get('firstFile', {}).get('name')
        f2 = clone.get('secondFile', {}).get('name')
        lines = clone.get('lines', 0)
        if lines >= 15:
            long_clones.append(
                f"{f1} lines {clone['firstFile']['start']}-{clone['firstFile']['end']} and {f2} lines {clone['secondFile']['start']}-{clone['secondFile']['end']} ({lines} lines)"
            )

    if long_clones:
        print('❌ Code duplication check failed! Duplication of >=15 lines is not allowed across any files.')
        for c in long_clones:
            print('  -', c)
        failed = True
    else:
        print('✅ No duplications of >=15 lines found.')

    # 3. 不允许任何重复 3 次及以上的代码块出现
    intervals = []
    for idx, clone in enumerate(duplicates):
        intervals.append({
            'id': f"clone_{idx}_first",
            'file': clone.get('firstFile', {}).get('name'),
            'start': clone.get('firstFile', {}).get('start'),
            'end': clone.get('firstFile', {}).get('end')
        })
        intervals.append({
            'id': f"clone_{idx}_second",
            'file': clone.get('secondFile', {}).get('name'),
            'start': clone.get('secondFile', {}).get('start'),
            'end': clone.get('secondFile', {}).get('end')
        })

    # 并查集实现
    parent = {}
    def find(i):
        if parent[i] == i:
            return i
        parent[i] = find(parent[i])
        return parent[i]

    def union(i, j):
        root_i = find(i)
        root_j = find(j)
        if root_i != root_j:
            parent[root_i] = root_j

    for item in intervals:
        parent[item['id']] = item['id']

    # 合并同一文件中且有重叠的区间
    for i in range(len(intervals)):
        for j in range(i + 1, len(intervals)):
            item_i = intervals[i]
            item_j = intervals[j]
            if item_i['file'] == item_j['file']:
                # 检查交集
                if max(item_i['start'], item_j['start']) <= min(item_i['end'], item_j['end']):
                    union(item_i['id'], item_j['id'])

    # 建立关联：在合并后的位置节点之间加边
    adj = {}
    for idx, clone in enumerate(duplicates):
        u = find(f"clone_{idx}_first")
        v = find(f"clone_{idx}_second")
        if u != v:
            adj.setdefault(u, set()).add(v)
            adj.setdefault(v, set()).add(u)

    # 寻找关联连通分量
    visited = set()
    components = []
    representatives = set(find(item['id']) for item in intervals)

    for node in representatives:
        if node not in visited:
            comp = []
            queue = [node]
            visited.add(node)
            while queue:
                curr = queue.pop(0)
                comp.append(curr)
                for neighbor in adj.get(curr, []):
                    if neighbor not in visited:
                        visited.add(neighbor)
                        queue.append(neighbor)
            components.append(comp)

    # 对每个连通分量，统计其对应的不同物理位置
    rep_to_info = {}
    for item in intervals:
        r = find(item['id'])
        if r not in rep_to_info:
            rep_to_info[r] = {'file': item['file'], 'ranges': []}
        rep_to_info[r]['ranges'].append((item['start'], item['end']))

    for r, info in rep_to_info.items():
        min_start = min(x[0] for x in info['ranges'])
        max_end = max(x[1] for x in info['ranges'])
        info['display'] = f"{info['file']} lines {min_start}-{max_end}"

    multi_clones = []
    for comp in components:
        if len(comp) >= 3:
            locs = [rep_to_info[r]['display'] for r in comp]
            multi_clones.append(locs)

    if multi_clones:
        print('❌ Code duplication check failed! Duplication of 3 or more times is not allowed.')
        for index, locs in enumerate(multi_clones):
            print(f"  - Duplicate Set #{index + 1} (repeated {len(locs)} times):")
            for loc in locs:
                print(f"    👉 {loc}")
        failed = True
    else:
        print('✅ No duplications of 3 or more times found.')

    if failed:
        sys.exit(1)

def check_wasm_sizes():
    import os
    limit_bytes = 2 * 1024 * 1024
    wasm_dirs = [
        'target/wasm32-unknown-unknown/release',
        'target/wasm32-unknown-unknown/debug'
    ]
    
    found_any = False
    failed = False
    
    for wasm_dir in wasm_dirs:
        if os.path.exists(wasm_dir):
            for root, _, files in os.walk(wasm_dir):
                for file in files:
                    if file.endswith('.wasm'):
                        found_any = True
                        path = os.path.join(root, file)
                        size = os.path.getsize(path)
                        if size > limit_bytes:
                            print(f"❌ WASM size check failed! {file} is {size / 1024 / 1024:.2f}MB, which exceeds the 2MB limit.")
                            failed = True
                        else:
                            print(f"✅ WASM size check passed for {file} ({size / 1024 / 1024:.2f}MB).")
    
    if failed:
        sys.exit(1)
    elif found_any:
        print("✅ WASM size checks complete.")

def check_unsafe_code():
    import os
    import re
    failed = False
    unsafe_pattern = re.compile(r'\bunsafe\b')
    
    for root, _, files in os.walk('crates'):
        if 'cowen-sys' in root or 'wasm' in root or 'tests' in root:
            continue
            
        for file in files:
            if file.endswith('.rs'):
                path = os.path.join(root, file)
                try:
                    with open(path, 'r', encoding='utf-8') as f:
                        lines = f.readlines()
                except Exception:
                    continue
                
                in_test_mod = False
                for line_idx, line in enumerate(lines):
                    if 'mod tests' in line or '#[cfg(test)]' in line:
                        in_test_mod = True
                    
                    if 'unsafe-allowed' in line:
                        continue
                        
                    clean_line = line.split('//')[0]
                    if '/*' in clean_line:
                        clean_line = clean_line.split('/*')[0]
                    
                    if not in_test_mod and unsafe_pattern.search(clean_line):
                        if clean_line.count('"') % 2 == 0:
                            print(f"❌ Unsafe code check failed! Found 'unsafe' block at {path}:{line_idx + 1}: {line.strip()}")
                            failed = True
                            
    if failed:
        sys.exit(1)
    else:
        print("✅ Unsafe code check passed (no raw unsafe code outside cowen-sys, wasm adapters, and test modules).")

if __name__ == '__main__':
    check_duplication()
    check_wasm_sizes()
    check_unsafe_code()
