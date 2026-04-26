"""Stage agents for the issue-resolver orchestrator.

Each stage exports a single agent class whose ``run`` method takes a
packaged context and returns the artifact for that stage (or raises a
stage-specific exception that the orchestrator knows how to translate
into labels + comments).
"""

from .requirements import (
    AgentError,
    IncompleteIssueError,
    RequirementsAgent,
)

__all__ = [
    "AgentError",
    "IncompleteIssueError",
    "RequirementsAgent",
]
