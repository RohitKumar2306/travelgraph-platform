#!/usr/bin/env python3
"""Request an advisory AI schema review and write a PR comment body."""

from __future__ import annotations

import argparse
import json
import subprocess
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


SERVICE_DIRS = {
    "property-service": "services/property-service",
    "pricing-service": "services/pricing-service",
    "booking-service": "services/booking-service",
    "user-service": "services/user-service",
    "review-service": "services/review-service",
}
SCHEMA_SUFFIXES = (".graphql", ".graphqls", ".gql")


def run_git(args: list[str], check: bool = True) -> str:
    proc = subprocess.run(["git", *args], text=True, capture_output=True, check=False)
    if check and proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"git {' '.join(args)} failed")
    return proc.stdout


def changed_files(base: str, head: str) -> list[str]:
    try:
        output = run_git(["diff", "--name-only", f"{base}...{head}"])
    except RuntimeError:
        output = run_git(["diff", "--name-only", base, head])
    return [line.strip() for line in output.splitlines() if line.strip()]


def is_schema_file(path: str) -> bool:
    return path.endswith(SCHEMA_SUFFIXES)


def first_changed_service(files: list[str]) -> str | None:
    for service, directory in SERVICE_DIRS.items():
        prefix = f"{directory}/"
        if any(path.startswith(prefix) for path in files):
            return service
    return None


def worktree_sdl(service: str) -> str:
    root = Path(SERVICE_DIRS[service])
    paths = sorted(path for path in root.rglob("*") if path.is_file() and is_schema_file(str(path)))
    return "\n\n".join(path.read_text(encoding="utf-8") for path in paths)


def ref_sdl(service: str, ref: str) -> str:
    directory = SERVICE_DIRS[service]
    output = run_git(["ls-tree", "-r", "--name-only", ref, directory], check=False)
    chunks: list[str] = []
    for path in output.splitlines():
        if is_schema_file(path):
            chunk = run_git(["show", f"{ref}:{path}"], check=False)
            if chunk:
                chunks.append(chunk)
    return "\n\n".join(chunks)


def post_review(assistant_url: str, payload: dict[str, Any]) -> str:
    body = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        f"{assistant_url.rstrip('/')}/review",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=45) as response:
        data = json.loads(response.read().decode("utf-8"))
    return data.get("markdown") or "AI review unavailable"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--assistant-url", default="http://localhost:8091")
    parser.add_argument("--out", default="ai-review-comment.md")
    args = parser.parse_args()

    service = first_changed_service(changed_files(args.base, args.head))
    if service is None:
        markdown = "## AI Schema Review (advisory)\n\nNo subgraph changes were detected.\n"
    else:
        old_schema = ref_sdl(service, args.base)
        new_schema = worktree_sdl(service)
        if not old_schema and not new_schema:
            markdown = (
                "## AI Schema Review (advisory)\n\n"
                f"`{service}` changed, but no checked-in SDL file was found for review.\n"
            )
        else:
            try:
                review = post_review(
                    args.assistant_url,
                    {
                        "oldSchema": old_schema,
                        "newSchema": new_schema,
                        "serviceName": service,
                        "ownerTeam": "platform",
                    },
                )
            except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, json.JSONDecodeError):
                review = "AI review unavailable"
            markdown = f"## AI Schema Review (advisory)\n\n{review.strip()}\n"

    Path(args.out).write_text(markdown, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
