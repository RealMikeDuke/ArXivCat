"""Comprehensive CLI test suite. Run with: python test_cli.py"""
from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

CLI = [sys.executable, "cli.py"]
WORKSPACE = "/mnt/d/Research/want_to_read"
TEST_ID = "2605.23889"  # HorizonStream - known to exist in workspace
BASE_ARGS = ["--workspace", WORKSPACE]

PASS = 0
FAIL = 0


def run(args: list[str], timeout: int = 30) -> tuple[int, str, str]:
    """Run CLI command and return (exit_code, stdout, stderr)."""
    try:
        r = subprocess.run(
            CLI + BASE_ARGS + args,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            cwd=str(Path(__file__).parent),
        )
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT"
    except Exception as e:
        return -1, "", str(e)


def check(name: str, exit_code: int, stdout: str | None, stderr: str | None,
          expect_exit: int = 0, expect_contains: str | None = None,
          expect_not_contains: str | None = None) -> None:
    global PASS, FAIL
    stdout = stdout or ""
    stderr = stderr or ""
    ok = True
    failures = []
    if exit_code != expect_exit:
        failures.append(f"exit code {exit_code} != {expect_exit}")
        ok = False
    if expect_contains and expect_contains not in stdout:
        failures.append(f"missing '{expect_contains[:80]}'")
        ok = False
    if expect_not_contains and expect_not_contains in stdout:
        failures.append(f"found unwanted '{expect_not_contains[:80]}'")
        ok = False
    if ok:
        print(f"  PASS: {name}")
        PASS += 1
    else:
        print(f"  FAIL: {name} -- {'; '.join(failures)}")
        if stdout:
            print(f"    stdout[:300]: {stdout[:300]}")
        if stderr:
            print(f"    stderr[:200]: {stderr[:200]}")
        FAIL += 1


# ═══════════════════════════════════════════════════════════════
# Test: help
# ═══════════════════════════════════════════════════════════════

print("=== Help ===")
ec, out, err = run(["--help"])
check("top-level help", ec, out, err, expect_contains="workspace")
check("top-level help has paper", ec, out, err, expect_contains="paper")
check("top-level help has chat", ec, out, err, expect_contains="chat")
check("top-level help has token", ec, out, err, expect_contains="token")

ec, out, err = run(["paper", "--help"])
check("paper help", ec, out, err, expect_contains="list")

ec, out, err = run(["chat", "--help"])
check("chat help", ec, out, err, expect_contains="side")

# ═══════════════════════════════════════════════════════════════
# Test: workspace
# ═══════════════════════════════════════════════════════════════

print("\n=== Workspace ===")
ec, out, err = run(["workspace", "open", WORKSPACE])
check("workspace open", ec, out, err, expect_contains="Workspace:")

ec, out, err = run(["workspace", "open", "Z:\\nonexistent\\path"])
check("workspace open nonexistent", ec, out, err, expect_exit=1, expect_contains="not found")

# ═══════════════════════════════════════════════════════════════
# Test: paper list
# ═══════════════════════════════════════════════════════════════

print("\n=== Paper List ===")
ec, out, err = run(["paper", "list"])
check("paper list", ec, out, err, expect_contains="arXiv ID")
check("paper list has papers", ec, out, err, expect_contains="2605.23889")

ec, out, err = run(["--workspace", WORKSPACE, "paper", "list"])
check("paper list with --workspace", ec, out, err, expect_contains="arXiv ID")

# ═══════════════════════════════════════════════════════════════
# Test: paper info
# ═══════════════════════════════════════════════════════════════

print("\n=== Paper Info ===")
ec, out, err = run(["paper", "info", TEST_ID])
check("paper info", ec, out, err, expect_contains="arXiv ID:")
check("paper info has id", ec, out, err, expect_contains="2605.23889")
check("paper info has title", ec, out, err, expect_contains="HorizonStream")
check("paper info has body", ec, out, err, expect_contains="body")
check("paper info has description", ec, out, err, expect_contains="description")

ec, out, err = run(["paper", "info", "nonexistent_id"])
check("paper info nonexistent", ec, out, err, expect_exit=1, expect_contains="Could not parse")

ec, out, err = run(["paper", "info", "2605.23889"])  # exact match
check("paper info exact match", ec, out, err, expect_contains="2605.23889")

# ═══════════════════════════════════════════════════════════════
# Test: paper preview
# ═══════════════════════════════════════════════════════════════

print("\n=== Paper Preview ===")
ec, out, err = run(["paper", "preview", TEST_ID, "--view", "body"])
check("paper preview body", ec, out, err, expect_contains="body.tex")

ec, out, err = run(["paper", "preview", TEST_ID, "--view", "description"])
check("paper preview description", ec, out, err, expect_contains="description.md")

ec, out, err = run(["paper", "preview", TEST_ID, "--view", "note"])
check("paper preview note", ec, out, err, expect_contains="note.txt")

ec, out, err = run(["paper", "preview", TEST_ID, "--view", "appendix"])
check("paper preview appendix", ec, out, err, expect_contains="appendix.tex")

ec, out, err = run(["paper", "preview", "nonexistent"])
check("paper preview nonexistent", ec, out, err, expect_exit=1, expect_contains="Could not parse")

# ═══════════════════════════════════════════════════════════════
# Test: paper note
# ═══════════════════════════════════════════════════════════════

print("\n=== Paper Note ===")
# Read existing note
ec, out, err = run(["paper", "note", TEST_ID])
check("paper note read", ec, out, err)

# Write note
ec, out, err = run(["paper", "note", TEST_ID, "Test note content from CLI"])
check("paper note write", ec, out, err, expect_contains="Note saved")

# Verify written
ec, out, err = run(["paper", "note", TEST_ID])
check("paper note verify", ec, out, err, expect_contains="Test note content from CLI")

# Clear note
ec, out, err = run(["paper", "note", TEST_ID, ""])
check("paper note clear", ec, out, err, expect_contains="Note saved")

# ═══════════════════════════════════════════════════════════════
# Test: token
# ═══════════════════════════════════════════════════════════════

print("\n=== Token ===")
ec, out, err = run(["token", "status"])
check("token status", ec, out, err, expect_contains="Token cached")

# ═══════════════════════════════════════════════════════════════
# Test: error handling
# ═══════════════════════════════════════════════════════════════

print("\n=== Error Handling ===")
ec, out, err = run(["nonexistent_command"])
check("nonexistent command", ec, out, err, expect_exit=2)

ec, out, err = run(["paper"])
check("paper without subcommand", ec, out, err, expect_exit=0)

# ═══════════════════════════════════════════════════════════════
# Test: _find_paper edge cases
# ═══════════════════════════════════════════════════════════════

print("\n=== Find Paper Edge Cases ===")
# Test with full URL
ec, out, err = run(["paper", "info", "https://arxiv.org/abs/2605.23889"])
check("paper info from URL", ec, out, err, expect_contains="2605.23889")

# Test with versioned ID
ec, out, err = run(["paper", "info", "2605.23889v1"])
check("paper info versioned", ec, out, err, expect_contains="2605.23889")

# Test with underscore format
ec, out, err = run(["paper", "info", "2605_23889"])
check("paper info underscore", ec, out, err, expect_contains="2605.23889")

# ═══════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════

print(f"\n{'='*60}")
print(f"Results: {PASS} passed, {FAIL} failed, {PASS + FAIL} total")
if FAIL > 0:
    print("SOME TESTS FAILED!")
    sys.exit(1)
else:
    print("All tests passed!")
    sys.exit(0)
