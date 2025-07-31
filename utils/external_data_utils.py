#!/usr/bin/env python3
"""
External Data Mount Utilities for Neural Trader

This module provides utilities for working with external drive mounts
in Docker containers, including validation, permission checking, and
data access helpers.
"""

import os
import sys
import stat
import logging
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Union
import json
from datetime import datetime

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class ExternalDataManager:
    """Manager for external data mounted in Docker containers."""
    
    def __init__(self, 
                 mount_path: Optional[str] = None,
                 enabled: Optional[bool] = None):
        """
        Initialize the external data manager.
        
        Args:
            mount_path: Path to the external data mount point
            enabled: Whether external data is enabled
        """
        self.mount_path = mount_path or os.environ.get('EXTERNAL_DATA_PATH', '/mnt/external-data')
        self.enabled = enabled if enabled is not None else \
                      os.environ.get('EXTERNAL_DATA_ENABLED', 'false').lower() == 'true'
        
        self.mount_path = Path(self.mount_path)
    
    def is_available(self) -> bool:
        """Check if external data is available and accessible."""
        if not self.enabled:
            logger.info("External data is disabled")
            return False
        
        if not self.mount_path.exists():
            logger.warning(f"External data path does not exist: {self.mount_path}")
            return False
        
        if not self.mount_path.is_dir():
            logger.warning(f"External data path is not a directory: {self.mount_path}")
            return False
        
        return True
    
    def check_permissions(self) -> Dict[str, Union[bool, str]]:
        """
        Check permissions on the external data mount.
        
        Returns:
            Dictionary with permission information
        """
        if not self.mount_path.exists():
            return {
                'exists': False,
                'readable': False,
                'writable': False,
                'executable': False,
                'error': 'Path does not exist'
            }
        
        try:
            path_stat = self.mount_path.stat()
            
            return {
                'exists': True,
                'readable': os.access(self.mount_path, os.R_OK),
                'writable': os.access(self.mount_path, os.W_OK),
                'executable': os.access(self.mount_path, os.X_OK),
                'owner_uid': path_stat.st_uid,
                'owner_gid': path_stat.st_gid,
                'permissions': oct(path_stat.st_mode)[-3:],
                'size_bytes': path_stat.st_size if self.mount_path.is_file() else None
            }
        except PermissionError as e:
            return {
                'exists': True,
                'readable': False,
                'writable': False,
                'executable': False,
                'error': f'Permission denied: {e}'
            }
        except Exception as e:
            return {
                'exists': True,
                'readable': False,
                'writable': False,
                'executable': False,
                'error': f'Error checking permissions: {e}'
            }
    
    def list_contents(self, 
                     max_depth: int = 2,
                     max_files: int = 100) -> Dict[str, Union[List, str]]:
        """
        List contents of the external data directory.
        
        Args:
            max_depth: Maximum directory depth to traverse
            max_files: Maximum number of files to list
            
        Returns:
            Dictionary with directory contents
        """
        if not self.is_available():
            return {'error': 'External data not available'}
        
        if not self.check_permissions()['readable']:
            return {'error': 'External data not readable'}
        
        try:
            contents = []
            file_count = 0
            
            for root, dirs, files in os.walk(self.mount_path):
                # Check depth
                depth = len(Path(root).relative_to(self.mount_path).parts)
                if depth >= max_depth:
                    dirs.clear()  # Don't descend further
                    continue
                
                # Add directories
                for dir_name in dirs[:10]:  # Limit directories shown
                    if file_count >= max_files:
                        break
                    
                    dir_path = Path(root) / dir_name
                    try:
                        dir_stat = dir_path.stat()
                        contents.append({
                            'name': str(dir_path.relative_to(self.mount_path)),
                            'type': 'directory',
                            'size': None,
                            'modified': datetime.fromtimestamp(dir_stat.st_mtime).isoformat(),
                            'permissions': oct(dir_stat.st_mode)[-3:]
                        })
                        file_count += 1
                    except (PermissionError, OSError):
                        continue
                
                # Add files
                for file_name in files:
                    if file_count >= max_files:
                        break
                    
                    file_path = Path(root) / file_name
                    try:
                        file_stat = file_path.stat()
                        contents.append({
                            'name': str(file_path.relative_to(self.mount_path)),
                            'type': 'file',
                            'size': file_stat.st_size,
                            'modified': datetime.fromtimestamp(file_stat.st_mtime).isoformat(),
                            'permissions': oct(file_stat.st_mode)[-3:]
                        })
                        file_count += 1
                    except (PermissionError, OSError):
                        continue
                
                if file_count >= max_files:
                    break
            
            return {
                'contents': contents,
                'total_listed': len(contents),
                'truncated': file_count >= max_files
            }
            
        except Exception as e:
            return {'error': f'Error listing contents: {e}'}
    
    def find_data_files(self, 
                       extensions: List[str] = None,
                       pattern: str = None) -> List[Path]:
        """
        Find data files in the external mount.
        
        Args:
            extensions: List of file extensions to look for (e.g., ['.csv', '.json'])
            pattern: Filename pattern to match
            
        Returns:
            List of matching file paths
        """
        if not self.is_available():
            return []
        
        if extensions is None:
            extensions = ['.csv', '.json', '.parquet', '.xlsx', '.txt']
        
        matching_files = []
        
        try:
            for ext in extensions:
                if pattern:
                    pattern_with_ext = f"*{pattern}*{ext}"
                else:
                    pattern_with_ext = f"*{ext}"
                
                matching_files.extend(self.mount_path.rglob(pattern_with_ext))
            
            return sorted(matching_files)
            
        except Exception as e:
            logger.error(f"Error finding data files: {e}")
            return []
    
    def get_file_info(self, relative_path: str) -> Dict[str, Union[str, int, bool]]:
        """
        Get information about a specific file in the external mount.
        
        Args:
            relative_path: Path relative to the mount point
            
        Returns:
            Dictionary with file information
        """
        if not self.is_available():
            return {'error': 'External data not available'}
        
        file_path = self.mount_path / relative_path
        
        if not file_path.exists():
            return {'error': f'File does not exist: {relative_path}'}
        
        try:
            file_stat = file_path.stat()
            
            return {
                'exists': True,
                'path': str(file_path),
                'relative_path': relative_path,
                'size': file_stat.st_size,
                'modified': datetime.fromtimestamp(file_stat.st_mtime).isoformat(),
                'created': datetime.fromtimestamp(file_stat.st_ctime).isoformat(),
                'is_file': file_path.is_file(),
                'is_directory': file_path.is_dir(),
                'readable': os.access(file_path, os.R_OK),
                'writable': os.access(file_path, os.W_OK),
                'permissions': oct(file_stat.st_mode)[-3:]
            }
            
        except Exception as e:
            return {'error': f'Error getting file info: {e}'}
    
    def read_file_safely(self, 
                        relative_path: str,
                        max_size_mb: int = 100) -> Tuple[bool, Union[str, bytes, None]]:
        """
        Safely read a file from the external mount.
        
        Args:
            relative_path: Path relative to the mount point
            max_size_mb: Maximum file size to read in MB
            
        Returns:
            Tuple of (success, content or error message)
        """
        if not self.is_available():
            return False, "External data not available"
        
        file_path = self.mount_path / relative_path
        
        if not file_path.exists():
            return False, f"File does not exist: {relative_path}"
        
        if not file_path.is_file():
            return False, f"Path is not a file: {relative_path}"
        
        try:
            file_size = file_path.stat().st_size
            max_size_bytes = max_size_mb * 1024 * 1024
            
            if file_size > max_size_bytes:
                return False, f"File too large: {file_size} bytes > {max_size_bytes} bytes"
            
            # Try to read as text first, fall back to binary
            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    content = f.read()
                return True, content
            except UnicodeDecodeError:
                with open(file_path, 'rb') as f:
                    content = f.read()
                return True, content
                
        except PermissionError:
            return False, f"Permission denied reading file: {relative_path}"
        except Exception as e:
            return False, f"Error reading file: {e}"
    
    def get_status_report(self) -> Dict:
        """
        Generate a comprehensive status report.
        
        Returns:
            Dictionary with complete status information
        """
        report = {
            'timestamp': datetime.now().isoformat(),
            'enabled': self.enabled,
            'mount_path': str(self.mount_path),
            'available': self.is_available()
        }
        
        if self.is_available():
            report['permissions'] = self.check_permissions()
            report['contents'] = self.list_contents(max_depth=1, max_files=20)
            
            # Find common data file types
            data_files = self.find_data_files()
            report['data_files'] = {
                'total_count': len(data_files),
                'sample_files': [str(f.relative_to(self.mount_path)) for f in data_files[:10]]
            }
            
            # Get file type statistics
            file_extensions = {}
            for file_path in data_files[:100]:  # Limit for performance
                ext = file_path.suffix.lower()
                file_extensions[ext] = file_extensions.get(ext, 0) + 1
            
            report['file_types'] = file_extensions
        
        return report


def main():
    """Command-line interface for external data utilities."""
    import argparse
    
    parser = argparse.ArgumentParser(description='Neural Trader External Data Utilities')
    parser.add_argument('--mount-path', help='Path to external data mount')
    parser.add_argument('--check', action='store_true', help='Check external data status')
    parser.add_argument('--list', action='store_true', help='List external data contents')
    parser.add_argument('--find', help='Find files matching pattern')
    parser.add_argument('--info', help='Get info about specific file')
    parser.add_argument('--report', action='store_true', help='Generate full status report')
    
    args = parser.parse_args()
    
    # Initialize manager
    manager = ExternalDataManager(mount_path=args.mount_path)
    
    if args.check or len(sys.argv) == 1:
        print("External Data Status:")
        print(f"  Enabled: {manager.enabled}")
        print(f"  Mount Path: {manager.mount_path}")
        print(f"  Available: {manager.is_available()}")
        
        if manager.is_available():
            perms = manager.check_permissions()
            print(f"  Readable: {perms['readable']}")
            print(f"  Writable: {perms['writable']}")
            print(f"  Permissions: {perms.get('permissions', 'unknown')}")
    
    if args.list:
        contents = manager.list_contents()
        if 'error' in contents:
            print(f"Error: {contents['error']}")
        else:
            print(f"\nContents ({contents['total_listed']} items):")
            for item in contents['contents']:
                size_str = f" ({item['size']} bytes)" if item['size'] else ""
                print(f"  {item['type']:>9}: {item['name']}{size_str}")
    
    if args.find:
        files = manager.find_data_files(pattern=args.find)
        print(f"\nFound {len(files)} matching files:")
        for file_path in files[:20]:  # Limit output
            print(f"  {file_path.relative_to(manager.mount_path)}")
        if len(files) > 20:
            print(f"  ... and {len(files) - 20} more")
    
    if args.info:
        info = manager.get_file_info(args.info)
        if 'error' in info:
            print(f"Error: {info['error']}")
        else:
            print(f"\nFile Info: {args.info}")
            for key, value in info.items():
                if key != 'error':
                    print(f"  {key}: {value}")
    
    if args.report:
        report = manager.get_status_report()
        print("\nFull Status Report:")
        print(json.dumps(report, indent=2, default=str))


if __name__ == '__main__':
    main()