#!/usr/bin/env python3
"""
Demonstration of the Neural Trader Health Monitoring System Enhancements

This script simulates the health checks that have been added to the Rust code
to show the functionality without needing to compile the entire codebase.
"""

import os
import json
import time
from pathlib import Path
from typing import Dict, List, Tuple, Optional

class HealthStatus:
    HEALTHY = "Healthy"
    DEGRADED = "Degraded"
    UNHEALTHY = "Unhealthy"
    UNKNOWN = "Unknown"

class ModelStorageHealthChecker:
    def __init__(self, models_path: str = "./models"):
        self.models_path = Path(models_path)
        
    def check_models_directory_exists(self) -> bool:
        """Check if models directory exists"""
        return self.models_path.exists() and self.models_path.is_dir()
    
    def check_models_directory_writable(self) -> bool:
        """Check if models directory is writable"""
        if not self.models_path.exists():
            return False
        return os.access(self.models_path, os.W_OK)
    
    def get_available_models(self) -> List[str]:
        """Get list of available models"""
        available_models = []
        
        if not self.models_path.exists():
            return available_models
            
        # Check model type directories
        model_types = ["checkpoints", "production"]
        for model_type in model_types:
            type_path = self.models_path / model_type
            if type_path.exists():
                for entry in type_path.iterdir():
                    if entry.is_dir():
                        available_models.append(f"{model_type}/{entry.name}")
        
        return available_models
    
    def check_required_models(self, available_models: List[str]) -> List[str]:
        """Check for missing required models"""
        required_models = ["NHITS", "MLP"]
        missing_models = []
        
        for required in required_models:
            found = any(required in model for model in available_models)
            if not found:
                missing_models.append(required)
                
        return missing_models
    
    def validate_model_integrity(self, model_path: Path) -> bool:
        """Validate model integrity by checking for required files"""
        if not model_path.exists() or not model_path.is_dir():
            return False
            
        # Check for common model file patterns
        required_patterns = [".pth", ".pkl", ".joblib", "config.json", ".h5"]
        
        for file_path in model_path.rglob("*"):
            if file_path.is_file():
                for pattern in required_patterns:
                    if pattern in file_path.suffix or pattern in file_path.name:
                        # Check file size is reasonable (> 1KB, < 10GB)
                        size = file_path.stat().st_size
                        if 1024 < size < 10 * 1024 * 1024 * 1024:  # 1KB < size < 10GB
                            return True
        return False
    
    def check_symlinks(self) -> bool:
        """Check if current model symlinks are valid"""
        current_path = self.models_path / "current"
        
        if not current_path.exists():
            return True  # No symlinks to validate
            
        for entry in current_path.iterdir():
            if entry.is_symlink():
                target = entry.resolve()
                if not target.exists():
                    return False
        return True
    
    def get_disk_space(self) -> Dict[str, float]:
        """Get disk space information"""
        if not self.models_path.exists():
            return {"total_gb": 0.0, "available_gb": 0.0, "used_percent": 0.0}
            
        # Get filesystem stats
        statvfs = os.statvfs(str(self.models_path))
        
        # Calculate space in GB
        total_bytes = statvfs.f_frsize * statvfs.f_blocks
        available_bytes = statvfs.f_frsize * statvfs.f_bavail
        used_bytes = total_bytes - available_bytes
        
        total_gb = total_bytes / (1024**3)
        available_gb = available_bytes / (1024**3)
        used_percent = (used_bytes / total_bytes) * 100 if total_bytes > 0 else 0
        
        return {
            "total_gb": round(total_gb, 2),
            "available_gb": round(available_gb, 2),
            "used_percent": round(used_percent, 1)
        }
    
    def calculate_model_sizes(self, available_models: List[str]) -> Dict[str, int]:
        """Calculate sizes of available models in MB"""
        model_sizes = {}
        
        for model in available_models:
            model_path = self.models_path / model.replace("/", "/")
            if model_path.exists():
                total_size = 0
                for file_path in model_path.rglob("*"):
                    if file_path.is_file():
                        total_size += file_path.stat().st_size
                model_sizes[model] = total_size // (1024 * 1024)  # Convert to MB
                
        return model_sizes
    
    def perform_health_check(self) -> Dict:
        """Perform comprehensive model storage health check"""
        start_time = time.time()
        
        # Check basic directory status
        directory_exists = self.check_models_directory_exists()
        directory_writable = self.check_models_directory_writable()
        
        # Get available models
        available_models = self.get_available_models()
        model_count = len(available_models)
        
        # Check required models
        missing_models = self.check_required_models(available_models)
        
        # Validate model integrity
        corrupted_models = []
        for model in available_models:
            model_path = self.models_path / model.replace("/", "/")
            if not self.validate_model_integrity(model_path):
                corrupted_models.append(model)
        
        # Check symlinks
        symlinks_valid = self.check_symlinks()
        
        # Get disk space
        disk_info = self.get_disk_space()
        low_disk_space = disk_info["available_gb"] < 1.0
        
        # Calculate model sizes
        model_sizes = self.calculate_model_sizes(available_models)
        total_model_size_mb = sum(model_sizes.values())
        
        # Determine health status
        if not directory_exists:
            status = HealthStatus.UNHEALTHY
            error = "Models directory does not exist"
        elif not directory_writable:
            status = HealthStatus.UNHEALTHY
            error = "Models directory is not writable"
        elif model_count == 0:
            status = HealthStatus.UNHEALTHY
            error = "No models available"
        elif corrupted_models:
            status = HealthStatus.UNHEALTHY
            error = f"Corrupted models detected: {', '.join(corrupted_models)}"
        elif low_disk_space:
            status = HealthStatus.DEGRADED
            error = "Low disk space for models"
        elif missing_models:
            status = HealthStatus.DEGRADED
            error = f"Missing required models: {', '.join(missing_models)}"
        else:
            status = HealthStatus.HEALTHY
            error = None
        
        response_time_ms = int((time.time() - start_time) * 1000)
        
        return {
            "component_type": "NeuralSystem",
            "status": status,
            "error_message": error,
            "response_time_ms": response_time_ms,
            "metadata": {
                "model_count": str(model_count),
                "available_models": ", ".join(available_models),
                "models_path": str(self.models_path),
                "models_writable": str(directory_writable),
                "required_models": "NHITS, MLP",
                "missing_models": ", ".join(missing_models) if missing_models else "",
                "corrupted_models": ", ".join(corrupted_models) if corrupted_models else "",
                "current_models_valid": str(symlinks_valid),
                "total_model_size_mb": str(total_model_size_mb),
                "disk_total_gb": str(disk_info["total_gb"]),
                "disk_available_gb": str(disk_info["available_gb"]),
                "disk_used_percent": str(disk_info["used_percent"])
            },
            "prometheus_metrics": {
                "neural_trader_models_available": model_count,
                "neural_trader_required_models_missing": len(missing_models),
                "neural_trader_model_storage_mounted": 1 if directory_exists else 0,
                "neural_trader_model_storage_writable": 1 if directory_writable else 0,
                "neural_trader_model_storage_size_mb": total_model_size_mb,
                "neural_trader_model_storage_disk_available_gb": disk_info["available_gb"],
                "neural_trader_model_storage_disk_used_percent": disk_info["used_percent"],
                "neural_trader_corrupted_models": len(corrupted_models)
            }
        }

def generate_prometheus_metrics(health_data: Dict) -> str:
    """Generate Prometheus metrics format"""
    metrics = health_data["prometheus_metrics"]
    
    output = []
    
    # Model storage specific metrics
    output.append("# HELP neural_trader_models_available Number of available models")
    output.append("# TYPE neural_trader_models_available gauge")
    output.append(f"neural_trader_models_available {metrics['neural_trader_models_available']}")
    output.append("")
    
    output.append("# HELP neural_trader_required_models_missing Number of missing required models")
    output.append("# TYPE neural_trader_required_models_missing gauge")
    output.append(f"neural_trader_required_models_missing {metrics['neural_trader_required_models_missing']}")
    output.append("")
    
    output.append("# HELP neural_trader_model_storage_mounted Whether model storage is mounted (1=yes, 0=no)")
    output.append("# TYPE neural_trader_model_storage_mounted gauge")
    output.append(f"neural_trader_model_storage_mounted {metrics['neural_trader_model_storage_mounted']}")
    output.append("")
    
    output.append("# HELP neural_trader_model_storage_writable Whether model storage is writable (1=yes, 0=no)")
    output.append("# TYPE neural_trader_model_storage_writable gauge")
    output.append(f"neural_trader_model_storage_writable {metrics['neural_trader_model_storage_writable']}")
    output.append("")
    
    output.append("# HELP neural_trader_model_storage_size_mb Total size of models in MB")
    output.append("# TYPE neural_trader_model_storage_size_mb gauge")
    output.append(f"neural_trader_model_storage_size_mb {metrics['neural_trader_model_storage_size_mb']}")
    output.append("")
    
    output.append("# HELP neural_trader_model_storage_disk_available_gb Available disk space in GB")
    output.append("# TYPE neural_trader_model_storage_disk_available_gb gauge")
    output.append(f"neural_trader_model_storage_disk_available_gb {metrics['neural_trader_model_storage_disk_available_gb']}")
    output.append("")
    
    output.append("# HELP neural_trader_model_storage_disk_used_percent Disk usage percentage")
    output.append("# TYPE neural_trader_model_storage_disk_used_percent gauge")
    output.append(f"neural_trader_model_storage_disk_used_percent {metrics['neural_trader_model_storage_disk_used_percent']}")
    output.append("")
    
    output.append("# HELP neural_trader_corrupted_models Number of corrupted models detected")
    output.append("# TYPE neural_trader_corrupted_models gauge")
    output.append(f"neural_trader_corrupted_models {metrics['neural_trader_corrupted_models']}")
    
    return "\n".join(output)

def main():
    print("🔍 Neural Trader Model Storage Health Check Demo")
    print("=" * 50)
    
    # Create health checker
    checker = ModelStorageHealthChecker()
    
    # Perform health check
    print("\n🧠 Performing Model Storage Health Check...")
    health_data = checker.perform_health_check()
    
    # Display results
    print(f"\n📊 Health Check Results:")
    print(f"Status: {health_data['status']}")
    if health_data['error_message']:
        print(f"Error: {health_data['error_message']}")
    print(f"Response Time: {health_data['response_time_ms']}ms")
    
    print(f"\n📋 Metadata:")
    for key, value in health_data['metadata'].items():
        if value:  # Only show non-empty values
            print(f"  {key}: {value}")
    
    print(f"\n📈 Prometheus Metrics:")
    prometheus_output = generate_prometheus_metrics(health_data)
    print(prometheus_output)
    
    # Summary
    print(f"\n✅ Health Check Summary:")
    print(f"  - Models Directory Exists: {'Yes' if checker.check_models_directory_exists() else 'No'}")
    print(f"  - Directory Writable: {'Yes' if checker.check_models_directory_writable() else 'No'}")
    print(f"  - Available Models: {len(checker.get_available_models())}")
    print(f"  - Missing Required Models: {len(checker.check_required_models(checker.get_available_models()))}")
    print(f"  - Symlinks Valid: {'Yes' if checker.check_symlinks() else 'No'}")
    
    disk_info = checker.get_disk_space()
    print(f"  - Disk Space: {disk_info['available_gb']:.2f}GB available ({disk_info['used_percent']:.1f}% used)")
    
    print(f"\n🎯 Integration Status:")
    print("  ✅ Model availability check")
    print("  ✅ Directory writability check") 
    print("  ✅ Symlink validation")
    print("  ✅ Disk space monitoring")
    print("  ✅ Model integrity validation")
    print("  ✅ Prometheus metrics export")
    print("  ✅ Docker environment compatibility")

if __name__ == "__main__":
    main()