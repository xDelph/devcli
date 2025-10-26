#!/bin/bash

echo "=== RustyCLI Demo ==="
echo ""

RUSTYCLI="./target/release/rustycli"

echo "1. Testing attached mode (quick command)..."
$RUSTYCLI start demo-echo --cmd "echo 'Hello from RustyCLI!'"
echo ""

echo "2. Starting detached process (30 second sleep)..."
$RUSTYCLI start demo-long --cmd "sleep 30" --detach
echo ""

echo "3. Starting detached process with custom directory and env vars..."
$RUSTYCLI start demo-env --cmd "printenv MY_VAR" --dir /tmp --env MY_VAR=TestValue --detach
echo ""

echo "4. Checking status of all processes..."
$RUSTYCLI status
echo ""

echo "5. Checking specific process status..."
$RUSTYCLI status demo-long
echo ""

echo "6. Log files created:"
ls -lh ~/.rustycli/logs/ | tail -5
echo ""

echo "7. PID files created:"
ls -lh ~/.rustycli/pids/
echo ""

echo "Demo complete! Detached processes are still running."
echo "Run '$RUSTYCLI status' to check them."

