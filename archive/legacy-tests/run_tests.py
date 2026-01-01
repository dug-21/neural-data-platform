#!/usr/bin/env python3
"""
Test Runner for Config Store Client

Provides comprehensive test execution with coverage reporting, following
TDD London School methodology. Includes test filtering, coverage analysis,
and performance reporting.
"""

import argparse
import sys
import subprocess
import os
from pathlib import Path
from typing import List, Optional


class TestRunner:
    """Test runner for config store client tests"""
    
    def __init__(self, base_path: Optional[Path] = None):
        self.base_path = base_path or Path(__file__).parent
        self.src_path = self.base_path.parent / "src" / "config-store"
        self.tests_path = self.base_path / "unit"
        
    def run_unit_tests(
        self,
        test_filter: str = "",
        coverage: bool = True,
        verbose: bool = True,
        fail_fast: bool = False
    ) -> int:
        """Run unit tests with coverage"""
        
        cmd = ["python", "-m", "pytest"]
        
        # Test path and filter
        if test_filter:
            cmd.append(f"{self.tests_path}::{test_filter}")
        else:
            cmd.append(str(self.tests_path))
            
        # Coverage options
        if coverage:
            cmd.extend([
                f"--cov={self.src_path}",
                "--cov-report=term-missing",
                "--cov-report=html:tests/coverage_html", 
                "--cov-report=xml:tests/coverage.xml",
                "--cov-fail-under=90",
                "--cov-branch"
            ])
            
        # Verbosity
        if verbose:
            cmd.append("-v")
            
        # Fail fast
        if fail_fast:
            cmd.append("-x")
            
        # Additional options
        cmd.extend([
            "--tb=short",
            "--strict-markers",
            "--timeout=30"
        ])
        
        print(f"Running: {' '.join(cmd)}")
        return subprocess.run(cmd).returncode
        
    def run_integration_tests(self, verbose: bool = True) -> int:
        """Run integration tests"""
        
        cmd = [
            "python", "-m", "pytest",
            str(self.tests_path / "test_config_client_integration.py"),
            "-m", "not slow"
        ]
        
        if verbose:
            cmd.append("-v")
            
        print(f"Running integration tests: {' '.join(cmd)}")
        return subprocess.run(cmd).returncode
        
    def run_performance_tests(self, verbose: bool = True) -> int:
        """Run performance tests"""
        
        cmd = [
            "python", "-m", "pytest", 
            str(self.tests_path),
            "-m", "performance",
            "--benchmark-only"
        ]
        
        if verbose:
            cmd.append("-v")
            
        print(f"Running performance tests: {' '.join(cmd)}")
        return subprocess.run(cmd).returncode
        
    def run_coverage_report(self) -> int:
        """Generate detailed coverage report"""
        
        # Run tests with coverage
        result = self.run_unit_tests(coverage=True, verbose=False)
        
        if result == 0:
            print("\n" + "="*60)
            print("COVERAGE REPORT GENERATED")
            print("="*60)
            print(f"HTML Report: {self.base_path / 'coverage_html' / 'index.html'}")
            print(f"XML Report: {self.base_path / 'coverage.xml'}")
            
            # Show coverage summary
            subprocess.run([
                "python", "-m", "coverage", "report",
                "--show-missing",
                "--skip-covered"
            ])
            
        return result
        
    def run_all_tests(self, verbose: bool = True) -> int:
        """Run all test suites"""
        
        print("Running complete test suite...")
        print("="*60)
        
        # Run unit tests
        print("\n1. Unit Tests")
        print("-" * 40)
        unit_result = self.run_unit_tests(verbose=verbose)
        
        if unit_result != 0:
            print("❌ Unit tests failed")
            return unit_result
            
        # Run integration tests
        print("\n2. Integration Tests")  
        print("-" * 40)
        integration_result = self.run_integration_tests(verbose=verbose)
        
        if integration_result != 0:
            print("❌ Integration tests failed")
            return integration_result
            
        print("\n" + "="*60)
        print("✅ ALL TESTS PASSED")
        print("="*60)
        
        return 0
        
    def lint_and_format(self) -> int:
        """Run code quality checks"""
        
        print("Running code quality checks...")
        
        # Check if tools are available
        tools = ["black", "isort", "flake8", "mypy"]
        missing_tools = []
        
        for tool in tools:
            if subprocess.run(["which", tool], capture_output=True).returncode != 0:
                missing_tools.append(tool)
                
        if missing_tools:
            print(f"⚠️  Missing tools: {', '.join(missing_tools)}")
            print("Install with: pip install black isort flake8 mypy")
            return 1
            
        # Run formatting
        print("\n1. Code Formatting (black)")
        black_result = subprocess.run([
            "black", "--check", "--diff", str(self.src_path), str(self.tests_path)
        ]).returncode
        
        # Run import sorting  
        print("\n2. Import Sorting (isort)")
        isort_result = subprocess.run([
            "isort", "--check-only", "--diff", str(self.src_path), str(self.tests_path)
        ]).returncode
        
        # Run linting
        print("\n3. Linting (flake8)")
        flake8_result = subprocess.run([
            "flake8", str(self.src_path), str(self.tests_path)
        ]).returncode
        
        # Run type checking
        print("\n4. Type Checking (mypy)")
        mypy_result = subprocess.run([
            "mypy", str(self.src_path)
        ]).returncode
        
        # Summary
        total_issues = sum([black_result, isort_result, flake8_result, mypy_result])
        
        if total_issues == 0:
            print("\n✅ All code quality checks passed")
        else:
            print(f"\n❌ {total_issues} code quality issues found")
            
        return total_issues


def main():
    """Main test runner entry point"""
    
    parser = argparse.ArgumentParser(description="Config Store Client Test Runner")
    parser.add_argument(
        "command",
        choices=["unit", "integration", "performance", "coverage", "all", "lint"],
        help="Test command to run"
    )
    parser.add_argument(
        "--filter", "-f",
        help="Test filter pattern (e.g., 'TestConnectionEstablishment')"
    )
    parser.add_argument(
        "--no-coverage", 
        action="store_true",
        help="Disable coverage reporting"
    )
    parser.add_argument(
        "--quiet", "-q",
        action="store_true", 
        help="Reduce output verbosity"
    )
    parser.add_argument(
        "--fail-fast", "-x",
        action="store_true",
        help="Stop on first failure"
    )
    
    args = parser.parse_args()
    
    runner = TestRunner()
    verbose = not args.quiet
    coverage = not args.no_coverage
    
    # Execute requested command
    if args.command == "unit":
        result = runner.run_unit_tests(
            test_filter=args.filter or "",
            coverage=coverage,
            verbose=verbose,
            fail_fast=args.fail_fast
        )
    elif args.command == "integration":
        result = runner.run_integration_tests(verbose=verbose)
    elif args.command == "performance":
        result = runner.run_performance_tests(verbose=verbose)
    elif args.command == "coverage":
        result = runner.run_coverage_report()
    elif args.command == "all":
        result = runner.run_all_tests(verbose=verbose)
    elif args.command == "lint":
        result = runner.lint_and_format()
    else:
        parser.print_help()
        result = 1
        
    return result


if __name__ == "__main__":
    sys.exit(main())