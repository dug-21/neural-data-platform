#!/usr/bin/env python3
"""
Simple coverage test runner for Alpaca provider that avoids complex dependencies.
This script directly tests the Alpaca provider module to measure coverage.
"""

import sys
import os
import subprocess

# Add the current directory to the path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

def run_coverage_test():
    """Run coverage test for Alpaca provider."""
    
    print("🔄 Starting Alpaca Provider Coverage Analysis...")
    
    # Run coverage on the specific module
    cmd = [
        sys.executable, "-m", "pytest", 
        "tests/test_alpaca_websocket.py",
        "--cov=providers.alpaca",
        "--cov-report=term-missing",
        "--cov-report=html:htmlcov",
        "--cov-fail-under=85",
        "-v", "--tb=short"
    ]
    
    print(f"🚀 Running command: {' '.join(cmd)}")
    
    try:
        result = subprocess.run(cmd, cwd="/workspaces/neural-trader/data_ingestion", 
                              capture_output=True, text=True, timeout=300)
        
        print("📊 COVERAGE TEST RESULTS:")
        print("=" * 60)
        print("STDOUT:")
        print(result.stdout)
        print("=" * 60)
        print("STDERR:")
        print(result.stderr)
        print("=" * 60)
        print(f"Return code: {result.returncode}")
        
        if result.returncode == 0:
            print("✅ Coverage test PASSED!")
            return True
        else:
            print("❌ Coverage test FAILED!")
            return False
            
    except subprocess.TimeoutExpired:
        print("⏰ Coverage test timed out after 5 minutes")
        return False
    except Exception as e:
        print(f"💥 Error running coverage test: {e}")
        return False

if __name__ == "__main__":
    success = run_coverage_test()
    sys.exit(0 if success else 1)