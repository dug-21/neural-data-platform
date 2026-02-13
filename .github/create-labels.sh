#!/bin/bash
# One-time script to create NDP GitHub labels
# Run: bash .github/create-labels.sh

set -e

gh label create "implementation" --color "0075ca" --description "Implementation tracking" --force
gh label create "air" --color "c5def5" --description "Air quality domain" --force
gh label create "dp" --color "bfdadc" --description "Data platform domain" --force
gh label create "fe" --color "d4c5f9" --description "Feature engineering" --force
gh label create "ops" --color "fef2c0" --description "Operations/tooling" --force
gh label create "ml" --color "f9d0c4" --description "ML/predictions" --force
gh label create "al" --color "e99695" --description "Alerts" --force
gh label create "db" --color "c2e0c6" --description "Dashboards" --force
gh label create "sparc:refinement" --color "fbca04" --description "SPARC refinement phase" --force
gh label create "sparc:completion" --color "fbca04" --description "SPARC completion phase" --force
gh label create "P0-critical" --color "b60205" --description "Critical priority" --force
gh label create "P1-high" --color "d93f0b" --description "High priority" --force
gh label create "P2-normal" --color "fbca04" --description "Normal priority" --force
gh label create "in-progress" --color "0e8a16" --description "Currently being worked on" --force
gh label create "blocked" --color "e4e669" --description "Blocked by dependency" --force
gh label create "needs-review" --color "7057ff" --description "Needs user review" --force

echo "All 16 labels created."
