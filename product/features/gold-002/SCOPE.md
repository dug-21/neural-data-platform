# gold-002: V1.2 Intelligence Foundation

## Vision

Build a domain-agnostic intelligence layer that embeds Gold layer features as vectors, performs K-NN similarity search to generate predictions, validates causal relationships via Granger testing, and proves the text embedding pipeline using NWS forecast data as a dogfood event stream.

## Tracking

- Feature: gold-002
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md`
- Architecture: `product/features/gold-002/ARCHITECTURE.md`
- Implementation roadmap: `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`

## Constraints

- Must run on Raspberry Pi 5 (16GB RAM, 1TB NVMe)
- Intelligence container must be separate from ingestion (workload isolation)
- No model training required for baseline predictions (K-NN is training-free)
- pgvector is the durable storage baseline; ruvector-core is an acceleration layer
- Phase 2 must deliver user-visible predictions after warmup period
- Architecture must accommodate V1.3 additions (SONA, sysops domain) without refactoring

## Out of Scope

- SONA learning (V1.3 — via ruvector's integrated SONA, not ruv-fann)
- MCP query interface (V1.3)
- Sysops/observability second domain (V1.3)
- Action framework and Q-Learning advisory (V1.3)
- Cross-domain intelligence (V2.0)
- ruv-fann (unnecessary — ruvector SONA subsumes it)
- Custom quantization (ruvector-core handles PQ internally)
