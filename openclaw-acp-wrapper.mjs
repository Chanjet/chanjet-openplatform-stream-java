#!/usr/bin/env node
import { spawn } from 'node:child_process';
const child = spawn('openclaw', ['--log-level', 'silent', 'acp'], { stdio: ['pipe', 'pipe', 'pipe'] });

child.stdout.on('data', (data) => {
  const lines = data.toString().split('\n');
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
      process.stdout.write(trimmed + '\n');
    }
  }
});

child.stderr.on('data', (data) => {
  process.stderr.write(data);
});

process.stdin.on('data', (data) => {
  child.stdin.write(data);
});

child.on('exit', (code) => {
  process.exit(code);
});
