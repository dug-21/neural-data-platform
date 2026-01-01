#!/usr/bin/env python3
"""
Redis Streams EventBus Test Runner

Comprehensive test runner for Redis Streams component tests with 
performance benchmarking and reporting capabilities.
"""

import argparse
import asyncio
import sys
import time
import json
from pathlib import Path
from typing import Dict, Any, List
import subprocess
import psutil
from dataclasses import dataclass, asdict


@dataclass
class TestRunResult:
    test_file: str
    duration_sec: float
    passed: int
    failed: int
    skipped: int
    coverage_percent: float
    memory_usage_mb: float
    performance_metrics: Dict[str, Any]


class RedisStreamsTestRunner:
    """Comprehensive test runner for Redis Streams tests."""
    
    def __init__(self):
        self.test_files = [
            'test_stream_channels.py',
            'test_message_routing.py', 
            'test_consumer_groups.py',
            'test_message_ordering.py',
            'test_throughput_benchmarks.py'
        ]
        self.results = []
        
    def run_all_tests(self, args) -> bool:
        """Run all test suites and generate report."""
        print("🚀 Starting Redis Streams EventBus Test Suite")
        print("=" * 60)
        
        total_start_time = time.time()
        all_passed = True
        
        for test_file in self.test_files:
            print(f"\n📋 Running {test_file}...")
            result = self.run_single_test_file(test_file, args)
            self.results.append(result)
            
            if result.failed > 0:
                all_passed = False
                
            self.print_test_summary(result)
        
        total_duration = time.time() - total_start_time
        
        # Generate comprehensive report
        self.generate_test_report(total_duration, args)
        
        return all_passed
    
    def run_single_test_file(self, test_file: str, args) -> TestRunResult:
        """Run a single test file and collect metrics."""
        start_time = time.time()
        initial_memory = psutil.Process().memory_info().rss / 1024 / 1024
        
        # Build pytest command
        cmd = ['pytest', test_file, '-v']
        
        if args.coverage:
            cmd.extend(['--cov=.', '--cov-report=term-missing'])
            
        if args.benchmark:
            cmd.extend(['--benchmark-only', '--benchmark-json=benchmark_results.json'])
            
        if args.markers:
            cmd.extend(['-m', args.markers])
            
        if args.parallel:
            cmd.extend(['-n', str(args.parallel)])
            
        # Run tests
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=Path(__file__).parent
            )
            
            # Parse results
            output_lines = result.stdout.split('\n')
            passed, failed, skipped = self.parse_test_results(output_lines)
            coverage_percent = self.parse_coverage_results(output_lines)
            
            # Parse benchmark results if available
            performance_metrics = {}
            if args.benchmark and Path('benchmark_results.json').exists():
                with open('benchmark_results.json', 'r') as f:
                    benchmark_data = json.load(f)
                    performance_metrics = self.extract_performance_metrics(benchmark_data)
                Path('benchmark_results.json').unlink()  # Cleanup
            
        except Exception as e:
            print(f"❌ Error running {test_file}: {e}")
            passed, failed, skipped = 0, 1, 0
            coverage_percent = 0.0
            performance_metrics = {}
        
        duration = time.time() - start_time
        final_memory = psutil.Process().memory_info().rss / 1024 / 1024
        memory_used = final_memory - initial_memory
        
        return TestRunResult(
            test_file=test_file,
            duration_sec=duration,
            passed=passed,
            failed=failed,
            skipped=skipped,
            coverage_percent=coverage_percent,
            memory_usage_mb=memory_used,
            performance_metrics=performance_metrics
        )
    
    def parse_test_results(self, output_lines: List[str]) -> tuple:
        """Parse pytest output to extract test counts."""
        passed = failed = skipped = 0
        
        for line in output_lines:
            if 'passed' in line and 'failed' in line:
                # Parse line like "5 passed, 2 failed, 1 skipped"
                parts = line.split()
                for i, part in enumerate(parts):
                    if part == 'passed' and i > 0:
                        passed = int(parts[i-1])
                    elif part == 'failed' and i > 0:
                        failed = int(parts[i-1])
                    elif part == 'skipped' and i > 0:
                        skipped = int(parts[i-1])
                break
            elif line.endswith('passed'):
                # Simple case: "5 passed"
                passed = int(line.split()[0])
                break
        
        return passed, failed, skipped
    
    def parse_coverage_results(self, output_lines: List[str]) -> float:
        """Parse coverage percentage from output."""
        for line in output_lines:
            if 'TOTAL' in line and '%' in line:
                # Extract percentage from coverage report
                parts = line.split()
                for part in parts:
                    if part.endswith('%'):
                        return float(part[:-1])
        return 0.0
    
    def extract_performance_metrics(self, benchmark_data: Dict[str, Any]) -> Dict[str, Any]:
        """Extract performance metrics from benchmark results."""
        metrics = {}
        
        if 'benchmarks' in benchmark_data:
            for benchmark in benchmark_data['benchmarks']:
                name = benchmark.get('name', 'unknown')
                stats = benchmark.get('stats', {})
                
                metrics[name] = {
                    'mean_sec': stats.get('mean', 0),
                    'min_sec': stats.get('min', 0),
                    'max_sec': stats.get('max', 0),
                    'stddev_sec': stats.get('stddev', 0),
                    'rounds': stats.get('rounds', 0)
                }
        
        return metrics
    
    def print_test_summary(self, result: TestRunResult):
        """Print summary for a single test file."""
        status_emoji = "✅" if result.failed == 0 else "❌"
        
        print(f"  {status_emoji} {result.test_file}")
        print(f"    Duration: {result.duration_sec:.2f}s")
        print(f"    Results: {result.passed} passed, {result.failed} failed, {result.skipped} skipped")
        
        if result.coverage_percent > 0:
            print(f"    Coverage: {result.coverage_percent:.1f}%")
            
        if result.memory_usage_mb > 0:
            print(f"    Memory: {result.memory_usage_mb:.2f} MB")
            
        if result.performance_metrics:
            print(f"    Performance: {len(result.performance_metrics)} benchmarks")
    
    def generate_test_report(self, total_duration: float, args):
        """Generate comprehensive test report."""
        print("\n" + "=" * 60)
        print("📊 TEST SUITE SUMMARY")
        print("=" * 60)
        
        total_passed = sum(r.passed for r in self.results)
        total_failed = sum(r.failed for r in self.results)
        total_skipped = sum(r.skipped for r in self.results)
        avg_coverage = sum(r.coverage_percent for r in self.results) / len(self.results)
        total_memory = sum(r.memory_usage_mb for r in self.results)
        
        print(f"Total Duration: {total_duration:.2f} seconds")
        print(f"Total Tests: {total_passed + total_failed + total_skipped}")
        print(f"  ✅ Passed: {total_passed}")
        print(f"  ❌ Failed: {total_failed}")
        print(f"  ⏭️  Skipped: {total_skipped}")
        print(f"Average Coverage: {avg_coverage:.1f}%")
        print(f"Memory Usage: {total_memory:.2f} MB")
        
        # Performance summary
        self.print_performance_summary()
        
        # Quality gates
        self.check_quality_gates(total_failed, avg_coverage)
        
        # Generate JSON report if requested
        if args.json_report:
            self.generate_json_report(total_duration)
    
    def print_performance_summary(self):
        """Print performance benchmark summary."""
        print(f"\n🏎️  PERFORMANCE SUMMARY")
        print("-" * 30)
        
        all_benchmarks = {}
        for result in self.results:
            all_benchmarks.update(result.performance_metrics)
        
        if not all_benchmarks:
            print("  No performance benchmarks run")
            return
        
        for name, metrics in all_benchmarks.items():
            print(f"  {name}:")
            print(f"    Mean: {metrics['mean_sec']*1000:.2f}ms")
            print(f"    Range: {metrics['min_sec']*1000:.2f}ms - {metrics['max_sec']*1000:.2f}ms")
            print(f"    Rounds: {metrics['rounds']}")
    
    def check_quality_gates(self, total_failed: int, avg_coverage: float):
        """Check quality gates and report status."""
        print(f"\n🚦 QUALITY GATES")
        print("-" * 20)
        
        gates_passed = 0
        total_gates = 3
        
        # Gate 1: No test failures
        if total_failed == 0:
            print("  ✅ No test failures")
            gates_passed += 1
        else:
            print(f"  ❌ {total_failed} test failures")
        
        # Gate 2: Coverage >= 85%
        if avg_coverage >= 85.0:
            print(f"  ✅ Coverage {avg_coverage:.1f}% >= 85%")
            gates_passed += 1
        else:
            print(f"  ❌ Coverage {avg_coverage:.1f}% < 85%")
        
        # Gate 3: All test files completed
        if len(self.results) == len(self.test_files):
            print("  ✅ All test files completed")
            gates_passed += 1
        else:
            print("  ❌ Some test files failed to complete")
        
        print(f"\nQuality Gates: {gates_passed}/{total_gates} passed")
        
        if gates_passed == total_gates:
            print("🎉 All quality gates passed!")
        else:
            print("⚠️  Some quality gates failed")
    
    def generate_json_report(self, total_duration: float):
        """Generate JSON report for CI/CD integration."""
        report = {
            'timestamp': time.time(),
            'total_duration_sec': total_duration,
            'results': [asdict(result) for result in self.results],
            'summary': {
                'total_passed': sum(r.passed for r in self.results),
                'total_failed': sum(r.failed for r in self.results),
                'total_skipped': sum(r.skipped for r in self.results),
                'average_coverage': sum(r.coverage_percent for r in self.results) / len(self.results),
                'total_memory_mb': sum(r.memory_usage_mb for r in self.results)
            }
        }
        
        with open('test_report.json', 'w') as f:
            json.dump(report, f, indent=2)
        
        print(f"\n📄 JSON report saved to test_report.json")
    
    def run_specific_tests(self, test_patterns: List[str], args) -> bool:
        """Run specific test patterns."""
        print(f"🎯 Running specific tests: {test_patterns}")
        
        cmd = ['pytest'] + test_patterns + ['-v']
        
        if args.coverage:
            cmd.extend(['--cov=.', '--cov-report=term-missing'])
        
        result = subprocess.run(cmd, cwd=Path(__file__).parent)
        return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(
        description="Redis Streams EventBus Test Runner"
    )
    
    parser.add_argument(
        '--coverage', 
        action='store_true',
        help='Enable coverage reporting'
    )
    
    parser.add_argument(
        '--benchmark',
        action='store_true', 
        help='Run performance benchmarks'
    )
    
    parser.add_argument(
        '--markers',
        type=str,
        help='Run tests with specific markers (e.g., "not slow")'
    )
    
    parser.add_argument(
        '--parallel',
        type=int,
        help='Run tests in parallel with N processes'
    )
    
    parser.add_argument(
        '--json-report',
        action='store_true',
        help='Generate JSON report for CI/CD'
    )
    
    parser.add_argument(
        '--specific',
        nargs='+',
        help='Run specific test files or patterns'
    )
    
    parser.add_argument(
        '--performance-only',
        action='store_true',
        help='Run only performance/throughput tests'
    )
    
    args = parser.parse_args()
    
    # Set default markers for performance-only mode
    if args.performance_only:
        args.markers = 'throughput or benchmark'
        args.benchmark = True
    
    runner = RedisStreamsTestRunner()
    
    try:
        if args.specific:
            success = runner.run_specific_tests(args.specific, args)
        else:
            success = runner.run_all_tests(args)
        
        return 0 if success else 1
        
    except KeyboardInterrupt:
        print("\n⏹️  Test run interrupted by user")
        return 130
    except Exception as e:
        print(f"\n💥 Test runner error: {e}")
        return 1


if __name__ == '__main__':
    sys.exit(main())