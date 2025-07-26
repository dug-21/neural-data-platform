"""Entry point for data_ingestion module CLI."""

import sys
from cli.backfill import main

if __name__ == "__main__":
    # Check if backfill command is specified
    if len(sys.argv) > 1 and sys.argv[1] == "backfill":
        # Remove 'backfill' from args as the CLI expects it as the module name
        sys.argv.pop(1)
    
    main()