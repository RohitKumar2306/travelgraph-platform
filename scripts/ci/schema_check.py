#!/usr/bin/env python3
"""Run deterministic GraphQL schema checks for CI pull requests."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
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
        return [
            line.strip()
            for line in run_git(["diff", "--name-only", f"{base}...{head}"]).splitlines()
            if line.strip()
        ]
    except RuntimeError:
        return [
            line.strip()
            for line in run_git(["diff", "--name-only", base, head]).splitlines()
            if line.strip()
        ]


def changed_services(files: list[str]) -> list[str]:
    services: list[str] = []
    for service, directory in SERVICE_DIRS.items():
        prefix = f"{directory}/"
        if any(path.startswith(prefix) for path in files):
            services.append(service)
    return services


def is_schema_file(path: str) -> bool:
    return path.endswith(SCHEMA_SUFFIXES)


def schema_paths_from_worktree(service: str) -> list[Path]:
    root = Path(SERVICE_DIRS[service])
    return sorted(path for path in root.rglob("*") if path.is_file() and is_schema_file(str(path)))


def schema_paths_from_ref(service: str, ref: str) -> list[str]:
    directory = SERVICE_DIRS[service]
    output = run_git(["ls-tree", "-r", "--name-only", ref, directory], check=False)
    return sorted(path for path in output.splitlines() if is_schema_file(path))


def read_schema_from_worktree(service: str) -> str:
    return "\n\n".join(path.read_text(encoding="utf-8") for path in schema_paths_from_worktree(service))


def read_schema_from_ref(service: str, ref: str) -> str:
    chunks: list[str] = []
    for path in schema_paths_from_ref(service, ref):
        output = run_git(["show", f"{ref}:{path}"], check=False)
        if output:
            chunks.append(output)
    return "\n\n".join(chunks)


FIELD_PATTERN = re.compile(r"^\s*([_A-Za-z][_0-9A-Za-z]*)\s*(?:\([^)]*\))?\s*:\s*([^#\n]+)")
TYPE_PATTERN = re.compile(r"\b(type|interface|input)\s+([_A-Za-z][_0-9A-Za-z]*)[^{]*\{([^}]*)\}", re.S)
ENUM_PATTERN = re.compile(r"\benum\s+([_A-Za-z][_0-9A-Za-z]*)[^{]*\{([^}]*)\}", re.S)


def normalize_type(value: str) -> str:
    return re.sub(r"\s+", "", value.strip())


def parse_fields(sdl: str) -> dict[str, dict[str, str]]:
    parsed: dict[str, dict[str, str]] = {}
    for _kind, type_name, body in TYPE_PATTERN.findall(sdl):
        fields: dict[str, str] = {}
        for line in body.splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith(("#", '"""', '"')):
                continue
            match = FIELD_PATTERN.match(line)
            if match:
                fields[match.group(1)] = normalize_type(match.group(2))
        parsed[type_name] = fields
    return parsed


def parse_enums(sdl: str) -> dict[str, set[str]]:
    parsed: dict[str, set[str]] = {}
    for enum_name, body in ENUM_PATTERN.findall(sdl):
        values = set()
        for line in body.splitlines():
            stripped = re.sub(r"#.*$", "", line).strip()
            if stripped and re.match(r"^[_A-Za-z][_0-9A-Za-z]*$", stripped):
                values.add(stripped)
        parsed[enum_name] = values
    return parsed


def local_breaking_changes(service: str, old_sdl: str, new_sdl: str) -> list[dict[str, Any]]:
    issues: list[dict[str, Any]] = []
    old_fields = parse_fields(old_sdl)
    new_fields = parse_fields(new_sdl)
    for type_name, fields in old_fields.items():
        if type_name not in new_fields:
            issues.append(issue("BREAKING", "TYPE_REMOVED", f"Type {type_name} was removed.", type_name))
            continue
        for field_name, old_type in fields.items():
            new_type = new_fields[type_name].get(field_name)
            if new_type is None:
                code = "MUTATION_REMOVED" if type_name == "Mutation" else "FIELD_REMOVED"
                issues.append(issue("BREAKING", code, f"{type_name}.{field_name} was removed.", type_name, field_name))
            elif new_type != old_type:
                code = "FIELD_BECAME_REQUIRED" if not old_type.endswith("!") and new_type.endswith("!") else "FIELD_TYPE_CHANGED"
                issues.append(
                    issue(
                        "BREAKING",
                        code,
                        f"{type_name}.{field_name} changed from {old_type} to {new_type}.",
                        type_name,
                        field_name,
                    )
                )

    old_enums = parse_enums(old_sdl)
    new_enums = parse_enums(new_sdl)
    for enum_name, old_values in old_enums.items():
        removed = sorted(old_values - new_enums.get(enum_name, set()))
        for value in removed:
            issues.append(issue("BREAKING", "ENUM_VALUE_REMOVED", f"{enum_name}.{value} was removed.", enum_name, value))

    return issues


def issue(severity: str, code: str, message: str, type_name: str | None = None, field_name: str | None = None) -> dict[str, Any]:
    return {
        "severity": severity,
        "code": code,
        "message": message,
        "typeName": type_name,
        "fieldName": field_name,
        "usageByClient": [],
    }


def post_json(url: str, payload: dict[str, Any], timeout: int = 20) -> dict[str, Any]:
    body = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def call_registry_check(registry_url: str, service: str, sdl: str) -> tuple[dict[str, Any] | None, str | None]:
    try:
        return post_json(f"{registry_url.rstrip('/')}/schemas/{service}/check", {"sdl": sdl}), None
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, json.JSONDecodeError) as exc:
        return None, str(exc)


def call_composer(registry_url: str) -> dict[str, Any]:
    try:
        return post_json(f"{registry_url.rstrip('/')}/supergraph/compose", {})
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, json.JSONDecodeError) as exc:
        return {"ok": False, "errors": [str(exc)]}


def merge_issues(*issue_lists: list[dict[str, Any]]) -> list[dict[str, Any]]:
    merged: list[dict[str, Any]] = []
    seen: set[tuple[str, str | None, str | None, str]] = set()
    for issues in issue_lists:
        for item in issues:
            key = (
                item.get("code", ""),
                item.get("typeName"),
                item.get("fieldName"),
                item.get("message", ""),
            )
            if key not in seen:
                seen.add(key)
                merged.append(item)
    return merged


def render_markdown(result: dict[str, Any]) -> str:
    lines = ["## Schema Check", ""]
    if not result["changedServices"]:
        lines.append("No subgraph service changes were detected.")
        return "\n".join(lines) + "\n"

    lines.append("Changed subgraphs: " + ", ".join(f"`{service}`" for service in result["changedServices"]))
    lines.append("")

    for check in result["checks"]:
        service = check["service"]
        lines.append(f"### `{service}`")
        if not check["schemaFound"]:
            lines.append("No checked-in SDL file was found for this service, so schema lint/breaking checks were skipped.")
            lines.append("")
            continue
        if check.get("registryError"):
            lines.append(f"Registry check unavailable; local diff fallback was used. `{check['registryError']}`")
        lint = check["lintIssues"]
        breaking = check["breakingChanges"]
        lines.append(f"- Lint issues: **{len(lint)}**")
        lines.append(f"- Breaking changes: **{len(breaking)}**")
        for item in breaking:
            lines.append(f"  - `{item.get('code', 'BREAKING')}` {item.get('message', '')}")
            usage = item.get("usageByClient") or []
            if usage:
                usage_summary = ", ".join(
                    f"{bucket.get('clientName', 'unknown')}@{bucket.get('clientVersion', 'unknown')}: {bucket.get('count', 0)}"
                    for bucket in usage
                )
                lines.append(f"    Recent usage: {usage_summary}")
        for item in lint[:10]:
            lines.append(f"  - `{item.get('code', 'LINT')}` {item.get('message', '')}")
        if len(lint) > 10:
            lines.append(f"  - ...and {len(lint) - 10} more lint issues.")
        lines.append("")

    composer = result.get("composer", {})
    lines.append("### Supergraph Composition")
    if composer.get("ok"):
        lines.append("Composition endpoint returned `ok: true`.")
    else:
        errors = composer.get("errors") or ["composition did not return ok"]
        lines.append("Composition endpoint did not complete successfully:")
        for error in errors:
            lines.append(f"- {error}")
    lines.append("")

    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--registry-url", default="")
    parser.add_argument("--out", default="schema-check-comment.md")
    parser.add_argument("--json", default="schema-check-result.json")
    args = parser.parse_args()

    files = changed_files(args.base, args.head)
    services = changed_services(files)
    result: dict[str, Any] = {
        "changedFiles": files,
        "changedServices": services,
        "checks": [],
        "breakingChanges": [],
        "lintIssues": [],
        "composer": {"ok": True, "errors": []},
    }

    for service in services:
        new_sdl = read_schema_from_worktree(service)
        old_sdl = read_schema_from_ref(service, args.base)
        check = {
            "service": service,
            "schemaFound": bool(new_sdl or old_sdl),
            "lintIssues": [],
            "breakingChanges": [],
            "registryError": None,
        }
        if new_sdl:
            local_breaking = local_breaking_changes(service, old_sdl, new_sdl) if old_sdl else []
            registry_response = None
            registry_error = None
            if args.registry_url:
                registry_response, registry_error = call_registry_check(args.registry_url, service, new_sdl)
            if registry_response is not None:
                check["lintIssues"] = registry_response.get("lintIssues", [])
                check["breakingChanges"] = merge_issues(registry_response.get("breakingChanges", []), local_breaking)
            elif old_sdl:
                check["breakingChanges"] = local_breaking
                check["registryError"] = registry_error
            elif registry_error:
                check["registryError"] = registry_error
        elif old_sdl:
            check["breakingChanges"] = local_breaking_changes(service, old_sdl, "")

        result["checks"].append(check)
        result["breakingChanges"].extend(check["breakingChanges"])
        result["lintIssues"].extend(check["lintIssues"])

    if args.registry_url:
        result["composer"] = call_composer(args.registry_url)

    Path(args.json).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    Path(args.out).write_text(render_markdown(result), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
