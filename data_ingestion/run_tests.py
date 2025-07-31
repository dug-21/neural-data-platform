#!/usr/bin/env python3
"""Test runner for WebSocket resilience features."""
import subprocess
import sys
import os

def run_tests():
    """Run the test suite with coverage reporting."""
    # Add project root to Python path
    project_root = os.path.dirname(os.path.abspath(__file__))
    sys.path.insert(0, project_root)
    
    print("Running WebSocket resilience tests...")
    print("=" * 60)
    
    # Run pytest with coverage
    cmd = [
        sys.executable, "-m", "pytest",
        "tests/test_websocket_resilience.py",
        "tests/test_alpaca_resilience_edge_cases.py",
        "--cov=utils.circuit_breaker",
        "--cov=providers.alpaca",
        "--cov-report=term-missing",
        "--cov-fail-under=85",
        "-v"
    ]
    
    result = subprocess.run(cmd, cwd=project_root)
    
    if result.returncode == 0:
        print("\n" + "=" * 60)
        print("✅ All tests passed with >85% coverage!")
        print("=" * 60)
    else:
        print("\n" + "=" * 60)
        print("❌ Tests failed or coverage below 85%")
        print("=" * 60)
        sys.exit(1)

if __name__ == "__main__":
    run_tests()