#!/usr/bin/env python3
"""
Integration Test Runner for Config-Store Integration Tests

This script orchestrates the complete test suite execution including:
- Environment setup validation
- Test execution with multiple scenarios
- Result collection and reporting
- Cleanup and teardown
"""

import asyncio
import json
import logging
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional

import click
import structlog


# Configure logging
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
    cache_logger_on_first_use=True,
)

logger = structlog.get_logger()

class TestExecutor:
    """Manages test execution and reporting."""
    
    def __init__(self, output_dir: Path):
        self.output_dir = output_dir
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.results = {}
        self.start_time = time.time()
    
    def run_command(self, command: List[str], env: Optional[Dict[str, str]] = None) -> subprocess.CompletedProcess:
        """Run a command and capture output."""
        logger.info("Running command", command=" ".join(command))
        
        full_env = os.environ.copy()
        if env:
            full_env.update(env)
        
        try:
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                env=full_env,
                timeout=600  # 10 minutes timeout
            )
            
            logger.info(
                "Command completed", 
                command=" ".join(command),
                return_code=result.returncode,
                stdout_lines=len(result.stdout.splitlines()) if result.stdout else 0,
                stderr_lines=len(result.stderr.splitlines()) if result.stderr else 0
            )
            
            return result
        
        except subprocess.TimeoutExpired:
            logger.error("Command timed out", command=" ".join(command))
            raise
        except Exception as e:
            logger.error("Command failed", command=" ".join(command), error=str(e))
            raise
    
    def run_test_scenario(self, scenario_name: str, docker_compose_args: List[str], pytest_args: List[str]) -> Dict:
        """Run a specific test scenario."""
        logger.info("Starting test scenario", scenario=scenario_name)
        
        scenario_start = time.time()
        scenario_results = {
            "name": scenario_name,
            "start_time": scenario_start,
            "status": "running"
        }
        
        try:
            # Start services for this scenario
            compose_cmd = ["docker-compose", "-f", "docker-compose.test.yml"] + docker_compose_args
            
            logger.info("Starting services", scenario=scenario_name)
            up_result = self.run_command(compose_cmd + ["up", "-d", "--build"])
            
            if up_result.returncode != 0:
                raise Exception(f"Failed to start services: {up_result.stderr}")
            
            # Wait for services to be ready
            logger.info("Waiting for services to be ready", scenario=scenario_name)
            time.sleep(30)  # Give services time to start
            
            # Run health checks
            health_check_result = self.run_command([
                "docker-compose", "-f", "docker-compose.test.yml", 
                "exec", "-T", "integration-test-runner", 
                "python", "/app/health_check.py"
            ])
            
            if health_check_result.returncode != 0:
                logger.warning("Health checks failed, but continuing", scenario=scenario_name)
            
            # Run the actual tests
            logger.info("Running tests", scenario=scenario_name)
            test_cmd = [
                "docker-compose", "-f", "docker-compose.test.yml",
                "exec", "-T", "integration-test-runner",
                "python", "-m", "pytest"
            ] + pytest_args
            
            test_result = self.run_command(test_cmd)
            
            # Collect test results
            logger.info("Collecting test results", scenario=scenario_name)
            self.collect_results(scenario_name)
            
            scenario_results.update({
                "status": "passed" if test_result.returncode == 0 else "failed",
                "return_code": test_result.returncode,
                "stdout": test_result.stdout,
                "stderr": test_result.stderr,
                "duration": time.time() - scenario_start
            })
            
        except Exception as e:
            logger.error("Test scenario failed", scenario=scenario_name, error=str(e))
            scenario_results.update({
                "status": "error",
                "error": str(e),
                "duration": time.time() - scenario_start
            })
        
        finally:
            # Clean up services
            logger.info("Cleaning up services", scenario=scenario_name)
            try:
                self.run_command(compose_cmd + ["down", "-v", "--remove-orphans"])
            except Exception as e:
                logger.warning("Cleanup failed", scenario=scenario_name, error=str(e))
        
        logger.info("Test scenario completed", scenario=scenario_name, status=scenario_results["status"])
        return scenario_results
    
    def collect_results(self, scenario_name: str):
        """Collect test results from containers."""
        try:
            # Copy test results from container
            copy_cmd = [
                "docker-compose", "-f", "docker-compose.test.yml",
                "cp", "integration-test-runner:/app/test-results", 
                str(self.output_dir / f"results-{scenario_name}")
            ]
            self.run_command(copy_cmd)
            
            # Copy coverage reports
            coverage_cmd = [
                "docker-compose", "-f", "docker-compose.test.yml",
                "cp", "integration-test-runner:/app/coverage",
                str(self.output_dir / f"coverage-{scenario_name}")
            ]
            self.run_command(coverage_cmd)
            
        except Exception as e:
            logger.warning("Failed to collect results", scenario=scenario_name, error=str(e))
    
    def generate_summary_report(self, results: List[Dict]) -> Dict:
        """Generate a comprehensive summary report."""
        total_duration = time.time() - self.start_time
        
        summary = {
            "test_run": {
                "start_time": self.start_time,
                "end_time": time.time(),
                "total_duration": total_duration,
                "total_scenarios": len(results)
            },
            "scenarios": results,
            "summary": {
                "passed": sum(1 for r in results if r["status"] == "passed"),
                "failed": sum(1 for r in results if r["status"] == "failed"),
                "error": sum(1 for r in results if r["status"] == "error")
            }
        }
        
        # Overall status
        if all(r["status"] == "passed" for r in results):
            summary["overall_status"] = "PASSED"
        elif any(r["status"] == "error" for r in results):
            summary["overall_status"] = "ERROR"
        else:
            summary["overall_status"] = "FAILED"
        
        return summary

@click.command()
@click.option('--scenario', multiple=True, 
              help='Specific scenarios to run (default: all)')
@click.option('--output-dir', default='./test-results', 
              help='Output directory for test results')
@click.option('--verbose', '-v', is_flag=True, 
              help='Verbose output')
@click.option('--no-cleanup', is_flag=True,
              help='Do not clean up containers after tests')
@click.option('--parallel', is_flag=True,
              help='Run scenarios in parallel (experimental)')
def main(scenario, output_dir, verbose, no_cleanup, parallel):
    """Run Config-Store Integration Tests."""
    
    # Configure logging level
    log_level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=log_level)
    
    output_path = Path(output_dir)
    executor = TestExecutor(output_path)
    
    # Define test scenarios
    scenarios = {
        "basic_integration": {
            "docker_compose_args": ["--profile", "integration"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestConfigurationLoading",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/basic-integration.xml"
            ]
        },
        "fallback_mechanism": {
            "docker_compose_args": ["--profile", "fallback"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestFallbackMechanism",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/fallback-mechanism.xml"
            ]
        },
        "hot_reloading": {
            "docker_compose_args": ["--profile", "integration"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestHotReloading",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/hot-reloading.xml"
            ]
        },
        "provider_configuration": {
            "docker_compose_args": ["--profile", "integration"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestProviderConfiguration",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/provider-configuration.xml"
            ]
        },
        "rate_limiting": {
            "docker_compose_args": ["--profile", "integration"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestRateLimitConfiguration",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/rate-limiting.xml"
            ]
        },
        "database_redis": {
            "docker_compose_args": ["--profile", "integration"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestDatabaseRedisConfiguration",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/database-redis.xml"
            ]
        },
        "migration_process": {
            "docker_compose_args": ["--profile", "full"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py::TestCompleteMigrationProcess",
                "-v", "--tb=short",
                "--junit-xml=/app/test-results/migration-process.xml"
            ]
        },
        "full_suite": {
            "docker_compose_args": ["--profile", "full"],
            "pytest_args": [
                "/app/tests/test_data_ingestion_config.py",
                "-v", "--tb=short",
                "--cov=/app/tests",
                "--cov-report=html:/app/coverage/html",
                "--cov-report=xml:/app/coverage/coverage.xml",
                "--junit-xml=/app/test-results/full-suite.xml"
            ]
        }
    }
    
    # Filter scenarios if specified
    if scenario:
        scenarios = {k: v for k, v in scenarios.items() if k in scenario}
    
    logger.info("Starting Config-Store Integration Test Suite", 
                scenarios=list(scenarios.keys()),
                output_dir=str(output_path))
    
    results = []
    
    try:
        # Run scenarios
        for scenario_name, scenario_config in scenarios.items():
            result = executor.run_test_scenario(
                scenario_name,
                scenario_config["docker_compose_args"],
                scenario_config["pytest_args"]
            )
            results.append(result)
            
            # Stop on first failure unless running all scenarios
            if result["status"] != "passed" and not scenario:
                logger.error("Stopping due to test failure", scenario=scenario_name)
                break
        
        # Generate summary report
        summary = executor.generate_summary_report(results)
        
        # Write summary to file
        summary_file = output_path / "summary.json"
        summary_file.write_text(json.dumps(summary, indent=2))
        
        # Print summary
        click.echo("\n" + "="*80)
        click.echo("CONFIG-STORE INTEGRATION TEST SUMMARY")
        click.echo("="*80)
        click.echo(f"Total Duration: {summary['test_run']['total_duration']:.2f} seconds")
        click.echo(f"Scenarios Run: {summary['test_run']['total_scenarios']}")
        click.echo(f"Passed: {summary['summary']['passed']}")
        click.echo(f"Failed: {summary['summary']['failed']}")
        click.echo(f"Errors: {summary['summary']['error']}")
        click.echo(f"Overall Status: {summary['overall_status']}")
        click.echo(f"Results Directory: {output_path}")
        click.echo("="*80)
        
        # Exit with appropriate code
        if summary["overall_status"] == "PASSED":
            sys.exit(0)
        else:
            sys.exit(1)
            
    except KeyboardInterrupt:
        logger.info("Test run interrupted by user")
        sys.exit(130)
    except Exception as e:
        logger.error("Test run failed", error=str(e))
        sys.exit(1)

if __name__ == "__main__":
    main()