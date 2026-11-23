"""Run a Lyquor or Foundry command with the same environment as the demo."""

from pathlib import Path


def lyquor_command(*arguments: str) -> list[str]:
    wrapper = Path(__file__).resolve().with_name("lyquor-env.sh")
    return ["bash", str(wrapper), *arguments]
