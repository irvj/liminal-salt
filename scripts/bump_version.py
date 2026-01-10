#!/usr/bin/env python3
"""
Version bump script for Liminal Salt.
Updates version in __init__.py, README.md, and package.json.
Generates changelog from git commits.
Commits and tags the release.
"""

import re
import json
import subprocess
import sys
from datetime import date
from pathlib import Path

# File paths (relative to project root)
INIT_FILE = Path("liminal_salt/__init__.py")
README_FILE = Path("README.md")
PACKAGE_JSON = Path("package.json")
CHANGELOG_FILE = Path("CHANGELOG.md")


def get_current_version():
    """Read current version from __init__.py"""
    content = INIT_FILE.read_text()
    match = re.search(r'__version__\s*=\s*["\']([^"\']+)["\']', content)
    if not match:
        raise ValueError("Could not find __version__ in __init__.py")
    return match.group(1)


def parse_version(version_str):
    """Parse version string into (major, minor, patch) tuple"""
    parts = version_str.split(".")
    return tuple(int(p) for p in parts[:3])


def bump_version(current, bump_type):
    """Calculate new version based on bump type"""
    major, minor, patch = parse_version(current)

    if bump_type == "major":
        return f"{major + 1}.0.0"
    elif bump_type == "minor":
        return f"{major}.{minor + 1}.0"
    elif bump_type == "patch":
        return f"{major}.{minor}.{patch + 1}"
    else:
        # Assume it's a specific version string
        return bump_type


def get_previous_tag():
    """Get the most recent git tag, or None if no tags exist"""
    try:
        result = subprocess.run(
            ["git", "describe", "--tags", "--abbrev=0"],
            capture_output=True,
            text=True,
            check=True
        )
        return result.stdout.strip()
    except subprocess.CalledProcessError:
        return None


def get_commits_since_tag(tag):
    """Get list of commit messages since the given tag (or all commits if no tag)"""
    if tag:
        cmd = ["git", "log", f"{tag}..HEAD", "--pretty=format:%s"]
    else:
        cmd = ["git", "log", "--pretty=format:%s"]

    result = subprocess.run(cmd, capture_output=True, text=True, check=True)

    if not result.stdout.strip():
        return []

    return result.stdout.strip().split("\n")


def generate_changelog_entry(version, commits):
    """Generate a changelog entry for the new version"""
    today = date.today().isoformat()

    lines = [f"## [{version}] - {today}", ""]

    if commits:
        lines.append("### Changes")
        for commit in commits:
            # Skip version bump commits
            if commit.startswith("Bump version"):
                continue
            # Clean up commit message
            commit = commit.strip()
            if commit:
                lines.append(f"- {commit}")
        lines.append("")
    else:
        lines.append("- Initial release")
        lines.append("")

    return "\n".join(lines)


def update_changelog(new_version):
    """Prepend new version entry to CHANGELOG.md"""
    prev_tag = get_previous_tag()
    commits = get_commits_since_tag(prev_tag)

    new_entry = generate_changelog_entry(new_version, commits)

    if CHANGELOG_FILE.exists():
        existing = CHANGELOG_FILE.read_text()
        # Check if there's a header we should preserve
        if existing.startswith("# Changelog"):
            # Find where the header ends (after the first blank line)
            header_end = existing.find("\n\n")
            if header_end != -1:
                header = existing[:header_end + 2]
                rest = existing[header_end + 2:]
                content = header + new_entry + "\n" + rest
            else:
                content = existing + "\n" + new_entry
        else:
            content = new_entry + "\n" + existing
    else:
        content = "# Changelog\n\n" + new_entry

    CHANGELOG_FILE.write_text(content)
    print(f"Updated: {CHANGELOG_FILE}")

    return commits


def update_init_file(new_version):
    """Update __init__.py"""
    content = INIT_FILE.read_text()
    new_content = re.sub(
        r'__version__\s*=\s*["\'][^"\']+["\']',
        f'__version__ = "{new_version}"',
        content
    )
    INIT_FILE.write_text(new_content)


def update_readme(new_version):
    """Update README.md version badge"""
    content = README_FILE.read_text()
    new_content = re.sub(
        r'\*\*v[\d.]+\*\*',
        f'**v{new_version}**',
        content
    )
    README_FILE.write_text(new_content)


def update_package_json(new_version):
    """Update package.json version"""
    data = json.loads(PACKAGE_JSON.read_text())
    data["version"] = new_version
    PACKAGE_JSON.write_text(json.dumps(data, indent=2) + "\n")


def git_commit_and_tag(version, no_commit=False, no_tag=False, push=False):
    """Commit changes and create git tag"""
    files = [
        str(INIT_FILE),
        str(README_FILE),
        str(PACKAGE_JSON),
        str(CHANGELOG_FILE)
    ]

    if no_commit:
        print("Skipping commit (--no-commit)")
        return

    # Stage files
    subprocess.run(["git", "add"] + files, check=True)

    # Commit
    subprocess.run(
        ["git", "commit", "-m", f"Bump version to {version}"],
        check=True
    )

    if no_tag:
        print("Skipping tag (--no-tag)")
    else:
        # Tag
        subprocess.run(
            ["git", "tag", f"v{version}"],
            check=True
        )
        print(f"Created tag: v{version}")

    if push:
        subprocess.run(["git", "push"], check=True)
        if not no_tag:
            subprocess.run(["git", "push", "--tags"], check=True)
        print("Pushed to remote")


def main():
    if len(sys.argv) < 2:
        print("Usage: python scripts/bump_version.py <patch|minor|major|X.Y.Z> [options]")
        print("\nOptions:")
        print("  --no-commit     Update files but don't commit or tag")
        print("  --no-tag        Commit but don't create a tag")
        print("  --no-changelog  Skip changelog generation")
        print("  --push          Push commit and tags to remote")
        sys.exit(1)

    bump_type = sys.argv[1]
    no_commit = "--no-commit" in sys.argv
    no_tag = "--no-tag" in sys.argv
    no_changelog = "--no-changelog" in sys.argv
    push = "--push" in sys.argv

    current = get_current_version()
    new_version = bump_version(current, bump_type)

    print(f"Bumping version: {current} -> {new_version}")

    # Update version files
    update_init_file(new_version)
    update_readme(new_version)
    update_package_json(new_version)
    print(f"Updated: {INIT_FILE}, {README_FILE}, {PACKAGE_JSON}")

    # Generate changelog
    if not no_changelog:
        commits = update_changelog(new_version)
        if commits:
            print(f"Changelog: {len(commits)} commits since last tag")

    # Git operations
    git_commit_and_tag(new_version, no_commit, no_tag, push)

    print(f"\n✅ Version bumped to {new_version}")


if __name__ == "__main__":
    main()
