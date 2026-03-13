# Implementation Protocol — RETIRED

This protocol has been replaced by:

- **`.claude/protocols/delivery-protocol.md`** — Session 2: three stages (component design → implementation → testing) with three validation gates.

The previous single-wave implementation model has been restructured into three stages within a unified delivery protocol:

- **Stage 3a**: Component design and pseudocode generation (was previously part of the planning protocol)
- **Stage 3b**: Code implementation (was the core of this protocol)
- **Stage 3c**: Testing and risk validation (was embedded in implementation, now a dedicated stage)

Each stage has its own validation gate. Gates auto-proceed on pass, stop on failure.

See `product/workflow/wf-002/002-proposed.md` for the full workflow design.
