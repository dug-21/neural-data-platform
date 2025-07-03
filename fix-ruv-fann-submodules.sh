#!/bin/bash

# Script to fix ruv-FANN submodule issues

echo "Fixing ruv-FANN submodule configuration..."

# Clone the repository without initializing submodules
git clone --no-checkout https://github.com/ruvnet/ruv-FANN.git ruv-fann-fixed
cd ruv-fann-fixed

# Checkout the branch
git checkout ruv-swarm-v1.05-daa

# Create proper .gitmodules file
cat > .gitmodules << 'EOF'
[submodule "claude-code-flow/claude-code-flow"]
	path = claude-code-flow/claude-code-flow
	url = https://github.com/ruvnet/claude-code-flow.git
[submodule "daa-repository"]
	path = daa-repository
	url = https://github.com/ruvnet/daa-repository.git
[submodule "polyglot-benchmark"]
	path = polyglot-benchmark
	url = https://github.com/ruvnet/polyglot-benchmark.git
EOF

# Remove submodule entries from git index
git rm --cached claude-code-flow/claude-code-flow
git rm --cached daa-repository
git rm --cached polyglot-benchmark

# Add .gitmodules
git add .gitmodules

# Commit the fix
git commit -m "fix: Add missing .gitmodules configuration

- Added proper .gitmodules file with URLs for all submodules
- Fixes 'no URL configured for submodule' error
- Enables proper dependency resolution in Cargo projects"

echo "Fix complete! The repository is now in ruv-fann-fixed/"
echo "You can push this to your fork and submit a PR to the upstream repository."