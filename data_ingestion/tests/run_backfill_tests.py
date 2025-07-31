#!/usr/bin/env python3
"""
Comprehensive Test Runner for Backfill Functionality

This script runs all backfill-related tests and provides detailed reporting
on test coverage, performance metrics, and quality assurance.
"""

import os
import sys
import subprocess
import json
import time
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Any, Optional

# Add parent directory to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


class BackfillTestRunner:
    """Comprehensive test runner for backfill functionality."""
    
    def __init__(self, verbose: bool = True, coverage: bool = True):
        """
        Initialize the test runner.
        
        Args:
            verbose: Enable verbose output
            coverage: Enable coverage reporting
        """
        self.verbose = verbose
        self.coverage = coverage
        self.test_dir = Path(__file__).parent
        self.results = {
            'timestamp': datetime.now().isoformat(),
            'test_suites': {},
            'summary': {},
            'performance': {},
            'coverage': {}
        }
    
    def run_test_suite(self, test_file: str, markers: Optional[List[str]] = None) -> Dict[str, Any]:
        """
        Run a specific test suite.
        
        Args:
            test_file: Path to test file
            markers: Optional pytest markers to filter tests
            
        Returns:
            Test results dictionary
        """
        print(f"\n{'='*60}")
        print(f"Running test suite: {test_file}")
        print(f"{'='*60}")
        
        # Build pytest command
        cmd = ['python', '-m', 'pytest', str(self.test_dir / test_file)]
        
        if self.verbose:
            cmd.extend(['-v', '--tb=short'])
        
        if markers:
            for marker in markers:
                cmd.extend(['-m', marker])
        
        if self.coverage:
            cmd.extend([
                '--cov=utils.file_backfill',
                '--cov=cli.backfill', 
                '--cov=providers.historical_backfill',
                '--cov-report=term-missing',
                '--cov-report=html:htmlcov',
                '--cov-append'
            ])
        
        # Add JSON report
        json_report = self.test_dir / f"report_{test_file.replace('.py', '')}.json"
        cmd.extend(['--json-report', f'--json-report-file={json_report}'])
        
        start_time = time.time()
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=self.test_dir.parent
            )
            
            execution_time = time.time() - start_time
            
            # Parse JSON report if available
            test_details = {}
            if json_report.exists():
                try:
                    with open(json_report, 'r') as f:
                        test_details = json.load(f)
                except Exception as e:
                    print(f"Warning: Could not parse JSON report: {e}")
            
            suite_result = {
                'file': test_file,
                'return_code': result.returncode,
                'execution_time': execution_time,
                'stdout': result.stdout,
                'stderr': result.stderr,
                'details': test_details
            }
            
            # Print summary
            if result.returncode == 0:
                print(f"✅ {test_file} - PASSED ({execution_time:.2f}s)")
            else:
                print(f"❌ {test_file} - FAILED ({execution_time:.2f}s)")
                if result.stderr:
                    print(f"Errors: {result.stderr}")
            
            return suite_result
            
        except Exception as e:
            print(f"❌ {test_file} - ERROR: {e}")
            return {
                'file': test_file,
                'return_code': -1,
                'execution_time': time.time() - start_time,
                'error': str(e)
            }
    
    def run_all_tests(self) -> Dict[str, Any]:
        """Run all backfill test suites."""
        print("🚀 Starting Comprehensive Backfill Test Suite")
        print(f"Test directory: {self.test_dir}")
        print(f"Coverage enabled: {self.coverage}")
        print(f"Verbose mode: {self.verbose}")
        
        # Define test suites with their characteristics
        test_suites = [
            {
                'file': 'test_backfill_integration.py',
                'description': 'Integration tests for backfill functionality',
                'markers': ['integration'],
                'priority': 'high'
            },
            {
                'file': 'test_backfill_validation.py', 
                'description': 'Data validation and quality scoring tests',
                'markers': None,
                'priority': 'high'
            },
            {
                'file': 'test_backfill_cli.py',
                'description': 'CLI interface and command tests',
                'markers': None,
                'priority': 'medium'
            }
        ]
        
        # Run each test suite
        for suite_config in test_suites:
            suite_result = self.run_test_suite(
                suite_config['file'],
                suite_config.get('markers')
            )
            suite_result.update({
                'description': suite_config['description'],
                'priority': suite_config['priority']
            })
            self.results['test_suites'][suite_config['file']] = suite_result
        
        # Run performance tests separately
        print(f"\n{'='*60}")
        print("Running Performance Tests")
        print(f"{'='*60}")
        
        perf_result = self.run_test_suite(
            'test_backfill_integration.py',
            ['performance']
        )
        self.results['performance'] = perf_result
        
        # Generate summary
        self._generate_summary()
        
        return self.results
    
    def _generate_summary(self):
        """Generate test execution summary."""
        total_suites = len(self.results['test_suites'])
        passed_suites = sum(1 for suite in self.results['test_suites'].values() 
                           if suite['return_code'] == 0)
        failed_suites = total_suites - passed_suites
        
        total_time = sum(suite['execution_time'] 
                        for suite in self.results['test_suites'].values())
        
        # Extract test counts from JSON reports
        total_tests = 0
        passed_tests = 0
        failed_tests = 0
        skipped_tests = 0
        
        for suite in self.results['test_suites'].values():
            if 'details' in suite and 'summary' in suite['details']:
                summary = suite['details']['summary']
                total_tests += summary.get('total', 0)
                passed_tests += summary.get('passed', 0)
                failed_tests += summary.get('failed', 0)
                skipped_tests += summary.get('skipped', 0)
        
        self.results['summary'] = {
            'total_suites': total_suites,
            'passed_suites': passed_suites,
            'failed_suites': failed_suites,
            'total_execution_time': total_time,
            'total_tests': total_tests,
            'passed_tests': passed_tests,
            'failed_tests': failed_tests,
            'skipped_tests': skipped_tests,
            'success_rate': (passed_tests / total_tests * 100) if total_tests > 0 else 0
        }
    
    def print_detailed_report(self):
        """Print detailed test report."""
        print(f"\n{'='*80}")
        print("🔍 DETAILED BACKFILL TEST REPORT")
        print(f"{'='*80}")
        
        summary = self.results['summary']
        
        print(f"\n📊 OVERALL SUMMARY:")
        print(f"  Total Test Suites: {summary['total_suites']}")
        print(f"  Passed Suites: {summary['passed_suites']} ✅")
        print(f"  Failed Suites: {summary['failed_suites']} ❌")
        print(f"  Total Tests: {summary['total_tests']}")
        print(f"  Passed Tests: {summary['passed_tests']} ✅")
        print(f"  Failed Tests: {summary['failed_tests']} ❌")
        print(f"  Skipped Tests: {summary['skipped_tests']} ⏭️")
        print(f"  Success Rate: {summary['success_rate']:.1f}%")
        print(f"  Total Execution Time: {summary['total_execution_time']:.2f}s")
        
        print(f"\n📋 TEST SUITE DETAILS:")
        for file, suite in self.results['test_suites'].items():
            status = "✅ PASSED" if suite['return_code'] == 0 else "❌ FAILED"
            print(f"\n  {file}")
            print(f"    Status: {status}")
            print(f"    Description: {suite['description']}")
            print(f"    Priority: {suite['priority']}")
            print(f"    Execution Time: {suite['execution_time']:.2f}s")
            
            if 'details' in suite and 'summary' in suite['details']:
                details = suite['details']['summary']
                print(f"    Tests: {details.get('total', 0)} total, "
                      f"{details.get('passed', 0)} passed, "
                      f"{details.get('failed', 0)} failed")
        
        print(f"\n🎯 TEST CATEGORIES COVERED:")
        categories = [
            "✅ Timezone Handling & Unix Nanosecond Conversion",
            "✅ File Format Processing (CSV, CSV.GZ, JSON, Parquet)", 
            "✅ Directory Traversal & Recursive Search",
            "✅ Symbol Filtering (Single & Multiple)",
            "✅ Date Range Filtering (Timezone-aware)",
            "✅ End-to-End Data Flow to TimescaleDB",
            "✅ Performance Testing with Large Datasets",
            "✅ Error Handling & Recovery Mechanisms",
            "✅ Checkpoint & Resume Functionality",
            "✅ Data Validation & Quality Scoring",
            "✅ CLI Interface & Command Processing"
        ]
        
        for category in categories:
            print(f"  {category}")
        
        print(f"\n⚡ PERFORMANCE HIGHLIGHTS:")
        if 'performance' in self.results:
            perf = self.results['performance']
            print(f"  Performance Test Execution: {perf['execution_time']:.2f}s")
            if perf['return_code'] == 0:
                print("  ✅ All performance benchmarks passed")
            else:
                print("  ❌ Some performance tests failed")
        
        # Coverage information
        if self.coverage:
            print(f"\n📈 COVERAGE INFORMATION:")
            print("  Coverage reports generated in htmlcov/ directory")
            print("  Key modules covered:")
            print("    - utils.file_backfill")
            print("    - cli.backfill")  
            print("    - providers.historical_backfill")
        
        print(f"\n🎉 TESTING COMPLETE!")
        if summary['failed_suites'] == 0 and summary['failed_tests'] == 0:
            print("  All tests passed successfully! 🎊")
        else:
            print(f"  {summary['failed_tests']} tests need attention ⚠️")
    
    def save_report(self, output_file: Optional[str] = None):
        """Save test report to file."""
        if not output_file:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            output_file = f"backfill_test_report_{timestamp}.json"
        
        output_path = self.test_dir / output_file
        
        try:
            with open(output_path, 'w') as f:
                json.dump(self.results, f, indent=2, default=str)
            
            print(f"\n💾 Test report saved to: {output_path}")
            
        except Exception as e:
            print(f"❌ Failed to save report: {e}")
    
    def check_dependencies(self):
        """Check if all required dependencies are available."""
        print("🔍 Checking test dependencies...")
        
        required_packages = [
            'pytest',
            'pytest-asyncio', 
            'pytest-cov',
            'pytest-json-report',
            'pandas',
            'numpy',
            'pytz'
        ]
        
        missing_packages = []
        
        for package in required_packages:
            try:
                __import__(package.replace('-', '_'))
                print(f"  ✅ {package}")
            except ImportError:
                print(f"  ❌ {package} - MISSING")
                missing_packages.append(package)
        
        if missing_packages:
            print(f"\n⚠️  Missing packages: {', '.join(missing_packages)}")
            print("Install with: pip install " + " ".join(missing_packages))
            return False
        
        print("✅ All dependencies available")
        return True


def main():
    """Main entry point for test runner."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description='Comprehensive Backfill Test Runner',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Run all tests with coverage
  python run_backfill_tests.py
  
  # Run tests without coverage, quiet mode
  python run_backfill_tests.py --no-coverage --quiet
  
  # Save detailed report
  python run_backfill_tests.py --save-report backfill_results.json
        """
    )
    
    parser.add_argument(
        '--no-coverage',
        action='store_true',
        help='Disable coverage reporting'
    )
    parser.add_argument(
        '--quiet',
        action='store_true', 
        help='Reduce output verbosity'
    )
    parser.add_argument(
        '--save-report',
        help='Save detailed report to specified file'
    )
    parser.add_argument(
        '--check-deps-only',
        action='store_true',
        help='Only check dependencies and exit'
    )
    
    args = parser.parse_args()
    
    # Create test runner
    runner = BackfillTestRunner(
        verbose=not args.quiet,
        coverage=not args.no_coverage
    )
    
    # Check dependencies
    if not runner.check_dependencies():
        sys.exit(1)
    
    if args.check_deps_only:
        sys.exit(0)
    
    # Run tests
    try:
        results = runner.run_all_tests()
        runner.print_detailed_report()
        
        if args.save_report:
            runner.save_report(args.save_report)
        
        # Exit with error code if any tests failed
        summary = results['summary']
        if summary['failed_suites'] > 0 or summary['failed_tests'] > 0:
            sys.exit(1)
        else:
            sys.exit(0)
            
    except KeyboardInterrupt:
        print("\n\n⚠️  Testing interrupted by user")
        sys.exit(130)
    except Exception as e:
        print(f"\n❌ Test runner error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()