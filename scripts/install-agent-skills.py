#!/usr/bin/env python3
"""Copy selected bundled skills without overwriting files or agent configuration."""
import argparse
import os
from pathlib import Path
import shutil
import sys

SKILLS_ROOT = Path(__file__).resolve().parents[1] / "skills"
SKILLS = ("e2evm-setup", "guest-e2e")


def destination(args):
    if args.dest is not None:
        return args.dest.expanduser().resolve()
    if args.agent == "codex":
        return (Path(os.environ.get("CODEX_HOME") or Path.home() / ".codex")
                .expanduser() / "skills").resolve()
    if args.agent == "claude":
        return (Path.home() / ".claude" / "skills").resolve()
    return None


def validate_source(name):
    source = SKILLS_ROOT / name
    if source.is_symlink() or not (source / "SKILL.md").is_file():
        raise ValueError(f"Bundled skill is missing or invalid: {source}")
    for entry in source.rglob("*"):
        if entry.is_symlink():
            raise ValueError(f"Bundled skill contains a symbolic link: {entry}")
        if not entry.is_file() and not entry.is_dir():
            raise ValueError(f"Bundled skill contains a special file: {entry}")
    return source


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        "Examples: --list | --agent codex --dry-run | "
        "--dest ./my-project/.claude/skills --skills guest-e2e. "
        "No default install target; no existing skill is ever replaced."))
    target = parser.add_mutually_exclusive_group()
    target.add_argument("--agent", choices=("codex", "claude"),
                        help="personal skills directory (Codex honors CODEX_HOME)")
    target.add_argument("--dest", type=Path,
                        help="explicit skills parent directory, including a project directory")
    parser.add_argument("--skills", nargs="+", choices=SKILLS, default=list(SKILLS),
                        help="skills to copy (default: both bundled skills)")
    parser.add_argument("--list", action="store_true", help="inventory; with a target, show collisions")
    parser.add_argument("--dry-run", action="store_true", help="validate and show copies without writing")
    args = parser.parse_args(argv)
    if args.list and args.dry_run:
        parser.error("choose --list or --dry-run")
    if len(set(args.skills)) != len(args.skills):
        parser.error("each skill may be selected only once")
    dest = destination(args)
    if dest is None and not args.list:
        parser.error("choose --agent or --dest (use --list to inspect first)")
    try:
        sources = [(name, validate_source(name)) for name in args.skills]
        if args.list:
            for name, source in sources:
                state = "bundled" if dest is None else (
                    "exists; will not overwrite" if os.path.lexists(dest / name) else "available")
                print(f"{name}: {state} ({source if dest is None else dest / name})")
            return 0
        if dest == SKILLS_ROOT or SKILLS_ROOT in dest.parents:
            raise ValueError("Destination must be outside the bundled skills directory")
        if dest.exists() and not dest.is_dir():
            raise ValueError(f"Destination is not a directory: {dest}")
        collisions = [str(dest / name) for name, _ in sources if os.path.lexists(dest / name)]
        if collisions:
            raise ValueError("Nothing installed; destinations already exist: " + ", ".join(collisions))
        for name, source in sources:
            print(f"{'Would install' if args.dry_run else 'Install'} {source} -> {dest / name}")
        if args.dry_run:
            return 0
        dest.mkdir(parents=True, exist_ok=True)
        for name, source in sources:
            # copytree exclusively creates the skill directory, so a concurrent
            # installer creating it after preflight also cannot be overwritten.
            shutil.copytree(source, dest / name,
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
        print("Installed. Reload your agent's skill list or start a new session.")
        return 0
    except (OSError, ValueError, shutil.Error) as error:
        print(f"Error: {error}", file=sys.stderr)
        print("Existing files were not replaced. Inspect the destination before retrying; "
              "an interrupted copy may leave a partial new skill.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
