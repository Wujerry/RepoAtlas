# Keep AI Memory under user control

Status: Superseded by [ADR 0022](0022-delegate-ai-work-to-external-agents.md)

Project conversations and regenerated summaries never mutate AI Memory silently. AI may propose a memory item, but only explicit user acceptance creates it, because durable operational knowledge such as deployment constraints or compatibility warnings must remain distinguishable from transient model output.

