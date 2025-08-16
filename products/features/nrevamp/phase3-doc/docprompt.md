Now, products/features/nrevamp/phase3 is in and running.

This project totally revamped neural-trader app. Create a hive-mind. Its mission, to revamp and update our /docs and README.md files.  

To understand full scope of what has been performed thus far:
    - products/features/nrevamp/HIGH_LEVEL_FEATURE_PLAN.md
    - products/features/nrevamp/analysis.md


Your mission is to fully understand and document what has been built thus far, and revamp the README.md, AND the docs/ directory to be 100% accruate.  

Our documentation must focus on a solid explanation of the overall architecture for the entire application. then, for each major component: data-ingestion, neural-trader details of their current capabilities.  This documentation needs to include configuration guides for each component.  The overall architecture diagrams need to include the fully operational deployment architecture, including our codebases, plus the supporting systems used within the app (database, metrics, grafana, redis, etc)

In addition we have a sophisticated docker deployment architecture.  This should be documented fully.  Right now, the only directory thats been fully tested is docker/production.

Organization:
I'm open to the hive-mind's consensus decision on how to organize this information.  We can have docs directories local to the component with detailed architecture, capabilites, and configuration guides, then keep the 'index' of these in project root, or you can centralized it.  My only requirement is that links extend from the projects readme to navigate it.

Cleanup:
It is also importent to consolidate any md files that are WRONG.  Leaving old doc creates confusion and bad decisions.  Consolidate any md files you find that are no longer accurate into an archive that I can review later.

Spawn a research and documentation hive-mind to fully revamp our documentation based on these requirements.  