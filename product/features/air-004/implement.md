Its time to begin implement buildout of this feature.

## Your Approach
You are to create a claude-flow swarm for implementation.  This is a London TDD based implementation that will align with much of the existing design and extend it, maintaining existing functionality.  Your team at a minimum:
- planner - To break down tasks and delegate
- tester - To ensure overall alignment of the test approach and strategy.  This should EXTEND not replace the existing test approach already implemnted.
- architect - To understand the SPARC Architecture, utilize the architecture namespace to save new designs and patterns, and to recall whats already been stored to ensure alignment
- coder - utilize claude-flow's coder to build the codebase, utilizing Reasoningbank architecture namespace to recall existing patterns and to save any new patterns created.
- docker specialist - Ensure the existing docker pattern is extended not rewritten.
- Reviewer - Ensure there are no stubs/todo's in the codebase, and to ensure the result aligns to the specification.

All Agents can utilze ReasoningBank in the architecture namespace to understand design patterns that exist.  You can query these through semantic searches.

## SPARC Documentation
All necessary artifacts are in the product/features/air-004 directory including.  Read these artifacts to understand the full scope before you initiate development.

## Completion
You are complete when you have built and successfully tested locally.  You don't have direct access to the PI, So you commit and push your work after successfully and fully testing locally.  I manually pull the codebase to the pi to deploy.
