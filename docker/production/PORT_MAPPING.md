# Production Port Mapping

## Port Assignments (VSCode Dev Container Compatible)

The production deployment uses non-conflicting ports to avoid issues with VSCode dev container services:

| Service | Internal Port | External Port | URL |
|---------|---------------|---------------|-----|
| Neural Trader API | 8080 | 8080 | http://localhost:8080 |
| Data Ingestion API | 8001 | 8001 | http://localhost:8001 |
| Data Ingestion Metrics | 9090 | 9091 | http://localhost:9091 |
| Prometheus | 9090 | 9090 | http://localhost:9090 |
| Grafana | 3000 | 3000 | http://localhost:3000 |
| TimescaleDB | 5432 | 5432 | localhost:5432 |
| Redis | 6379 | 6379 | localhost:6379 |

## VSCode Dev Container Conflicts Avoided

VSCode dev containers commonly use:
- Port 3000 (development servers) 
- Port 8000-8999 (development APIs)
- Port 9090 (various dev tools)

Our production mapping uses standard ports with VSCode auto-forwarding disabled:
- Port 3000 for Grafana (standard)
- Port 8080 for Neural Trader API (standard)
- Port 8001 for Data Ingestion API (standard)
- Port 9091 for Data Ingestion metrics (standard)
- Port 9090 for Prometheus (standard)

## Quick Access

After running `./deploy.sh`, access services at:
- **Main API**: http://localhost:8080
- **Data Ingestion**: http://localhost:8001
- **Monitoring**: http://localhost:9090 (Prometheus)
- **Dashboards**: http://localhost:3000 (Grafana)

All ports are bound to 127.0.0.1 for security (localhost only).