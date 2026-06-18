#!/usr/bin/env python3
# scripts/coverage_gate.py
# Parses llvm-cov report, displays a simplified per-crate coverage summary,
# and enforces a history-based dynamic quality gate value.

import os
import re
import sys
import argparse

line_re = re.compile(
    r'^.*?(?:/|^)(cli/cowen/crates/[^/]+/[^/]+|sdk/rust)\S*\s+(\d+)\s+(\d+)\s+[\d\.]+%?\s+(\d+)\s+(\d+)\s+[\d\.]+%?\s+(\d+)\s+(\d+)\s+(\d+\.\d+)?%?'
)

def parse_line(line, regex):
    match = regex.match(line)
    if match:
        path_prefix = match.group(1)
        if path_prefix.startswith("cli/cowen/crates/"):
            parts = path_prefix.split('/')
            crate_name = parts[-1]
        else:
            crate_name = "sdk-rust"
            
        line_count = int(match.group(6))
        line_uncovered = int(match.group(7))
        return crate_name, line_count, line_uncovered
    return None

def parse_total_coverage(line):
    if line.startswith('TOTAL'):
        parts = line.split()
        for part in reversed(parts):
            if '%' in part:
                try:
                    return float(part.replace('%', ''))
                except ValueError:
                    pass
    return None

def get_relative_path(line):
    idx = line.find("cli/cowen/crates/")
    if idx != -1:
        parts = line[idx:].split()
        if parts:
            return parts[0]
    idx = line.find("sdk/rust/")
    if idx != -1:
        parts = line[idx:].split()
        if parts:
            return parts[0]
    return None

def run_self_tests():
    print("🧪 Running self-tests (TDD Green Phase)...")
    # 1. Relative path should match successfully
    relative_line = "cli/cowen/crates/core/cowen-infra/src/mask.rs  222  193  13.06%  6  5  16.67%  134  119  11.19%"
    res_rel = parse_line(relative_line, line_re)
    assert res_rel == ("cowen-infra", 134, 119), f"Failed relative path test: expected ('cowen-infra', 134, 119), got {res_rel}"
    
    # 2. Absolute path should also match successfully now
    absolute_line_docker = "/workspace/cli/cowen/crates/core/cowen-infra/src/mask.rs  222  193  13.06%  6  5  16.67%  134  119  11.19%"
    res_abs_docker = parse_line(absolute_line_docker, line_re)
    assert res_abs_docker == ("cowen-infra", 134, 119), f"Failed absolute docker path test: expected ('cowen-infra', 134, 119), got {res_abs_docker}"
    
    # 3. TOTAL line parsing should correctly extract the last line coverage percentage
    total_line_1 = "TOTAL                                               108      81   25.00%      10       7   30.00%      74      59   20.27%"
    total_val_1 = parse_total_coverage(total_line_1)
    assert total_val_1 == 20.27, f"Failed TOTAL parsing test: expected 20.27, got {total_val_1}"
    
    # 4. Relative path extraction test
    res_path = get_relative_path(absolute_line_docker)
    assert res_path == "cli/cowen/crates/core/cowen-infra/src/mask.rs", f"Failed relative path extraction, got {res_path}"
    
    print("✅ All self-tests passed!")

def main():
    parser = argparse.ArgumentParser(description="Cowen Code Coverage Gate Enforcer")
    parser.add_argument("--report", default="target/coverage_report.txt", help="Path to llvm-cov report file")
    parser.add_argument("--gate", default=".coverage_gate", help="Path to stored coverage gate file")
    parser.add_argument("--self-test", action="store_true", help="Run self-tests")
    args = parser.parse_args()

    if args.self_test:
        run_self_tests()
        sys.exit(0)

    report_path = args.report
    gate_file_path = args.gate

    if not os.path.exists(report_path):
        print(f"❌ Error: Coverage report not found at {report_path}")
        sys.exit(1)

    file_data = {}

    with open(report_path, 'r') as f:
        for line in f:
            line = line.strip()
            match_res = parse_line(line, line_re)
            if match_res:
                crate_name, line_count, line_uncovered = match_res
                rel_path = get_relative_path(line)
                if not rel_path:
                    rel_path = line.split()[0]
                
                if rel_path not in file_data:
                    file_data[rel_path] = []
                
                file_data[rel_path].append({
                    'crate_name': crate_name,
                    'line_count': line_count,
                    'line_uncovered': line_uncovered
                })

    crates_data = {}
    for rel_path, records in file_data.items():
        best = max(records, key=lambda x: (x['line_count'] - x['line_uncovered']))
        crate_name = best['crate_name']
        if crate_name not in crates_data:
            crates_data[crate_name] = {'lines': 0, 'uncovered': 0}
        crates_data[crate_name]['lines'] += best['line_count']
        crates_data[crate_name]['uncovered'] += best['line_uncovered']

    grand_total = sum(d['lines'] for d in crates_data.values())
    grand_uncovered = sum(d['uncovered'] for d in crates_data.values())
    grand_covered = grand_total - grand_uncovered
    total_coverage = (grand_covered / grand_total * 100) if grand_total > 0 else 0.0

    # 2. Print Simplified per-crate coverage summary table if parsing multiple crates
    if crates_data:
        print(f"\n📈 {'Crate Name':<28} | {'Total Lines':<12} | {'Covered Lines':<13} | {'Line Coverage':<13}")
        print("-" * 75)
        for name, data in sorted(crates_data.items(), key=lambda x: x[0]):
            total = data['lines']
            uncovered = data['uncovered']
            covered = total - uncovered
            pct = (covered / total * 100) if total > 0 else 0
            print(f"{name:<28} | {total:<12} | {covered:<13} | {pct:>11.2f}%")
        print("-" * 75)

    print(f"📊 {'TOTAL OVERALL COVERAGE':<28} | {'':<12} | {'':<13} | {total_coverage:>11.2f}%")
    print("-" * 75)

    # 3. Dynamic Coverage Gate Check
    history_gate = 0.0
    if os.path.exists(gate_file_path):
        try:
            with open(gate_file_path, 'r') as gf:
                history_gate = float(gf.read().strip())
        except Exception as e:
            print(f"⚠️ Failed to read history gate from {gate_file_path}: {e}, resetting gate.")

    print(f"🔒 Current Coverage: {total_coverage:.2f}%")
    print(f"🔑 History Gate:     {history_gate:.2f}%")

    if total_coverage > history_gate:
        print(f"🎉 Coverage improved! Updating gate from {history_gate:.2f}% to {total_coverage:.2f}%")
        try:
            with open(gate_file_path, 'w') as gf:
                gf.write(f"{total_coverage:.2f}\n")
        except Exception as e:
            print(f"❌ Error writing new gate value to {gate_file_path}: {e}")
            sys.exit(1)
    elif abs(total_coverage - history_gate) < 1e-5:
        print(f"✅ Coverage matches the gate value.")
    else:
        print(f"❌ ERROR: Current coverage ({total_coverage:.2f}%) is BELOW the gate ({history_gate:.2f}%)!")
        sys.exit(1)

    sys.exit(0)

if __name__ == "__main__":
    main()
