#!/usr/bin/env python3
"""
Standalone script to run data backfill operations.

This script provides an easy way to run backfill operations without
needing to use the module syntax.
"""

import os
import sys

# Add parent directory to Python path
parent_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, parent_dir)

from data_ingestion.cli.backfill import main

if __name__ == "__main__":
    main()