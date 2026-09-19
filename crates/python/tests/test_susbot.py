"""Tests for the Python package. Run from crates/python after
`maturin develop` (or against an installed wheel): `pytest tests`."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import os

import pytest

import susbot

# The repository root, for the example files; CI sets it when the tests run
# from a copy outside the checkout (against an installed wheel).
REPO = Path(os.environ.get("SUSBOT_REPO") or Path(__file__).resolve().parents[3])
KITCHEN_SINK = REPO / "examples" / "kitchen-sink.robots.txt"

GPTBOT_BLOCKED = "User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nDisallow: /admin/\n"


def test_version_matches_the_engine():
    assert susbot.__version__
    out = subprocess.run([sys.executable, "-m", "susbot", "--version"], capture_output=True, encoding="utf-8", check=True)
    assert out.stdout.strip() == f"susbot {susbot.__version__}"


def test_access():
    a = susbot.Analysis(GPTBOT_BLOCKED, site_url="https://example.com/")
    assert not a.allowed("GPTBot", "/page")
    assert a.allowed("Googlebot", "/page")
    assert not a.allowed(["Googlebot"], "/admin/x")
    access = a.check_access("GPTBot", "/robots.txt")
    assert access["allowed"] and access["always"]


def test_report_shape():
    a = susbot.Analysis(KITCHEN_SINK.read_text(encoding="utf-8"), site_url="https://www.example.com/")
    report = a.report
    assert report["schemaVersion"]
    assert report["summary"]["platform"] == "WordPress"
    assert any(i["id"] == "parser.ruleBeforeAgent" for i in a.issues)
    assert any(f["severity"] == "high" for f in a.security)
    assert {c["name"] for c in a.crawlers} >= {"Googlebot", "GPTBot"}
    assert json.loads(a.report_json(pretty=True))["summary"] == report["summary"]
    assert a.markdown().startswith("#")
    assert "<html" in a.html()
    assert a.recommended_actions()
    tabs = a.csv_tabs()
    assert set(tabs) == set(a.tab_labels())
    assert a.tsv(next(iter(tabs))) is not None
    assert a.tsv("no-such-tab") is None
    assert a.filename()
    assert "Googlebot" in repr(a) or "rules=" in repr(a)


def test_config_and_errors():
    assert "[crawlers]" in susbot.default_config()
    susbot.validate_config('[rules]\ndisabled = ["seo.trailingSlash"]\n')
    with pytest.raises(ValueError):
        susbot.validate_config("[rules\n")
    with pytest.raises(ValueError):
        susbot.Analysis("User-agent: *", config="not = [valid")


def test_rule_overrides_apply():
    text = "User-agent: *\nDisallow: /admin\n"
    base = {i["id"] for i in susbot.Analysis(text).issues}
    assert base, "the fixture should raise at least one finding"
    disabled = sorted(base)[0]
    quiet = susbot.Analysis(text, config=f'[rules]\ndisabled = ["{disabled}"]\n')
    assert disabled not in {i["id"] for i in quiet.issues}


def test_diff():
    old = susbot.Analysis("User-agent: *\nAllow: /\n", site_url="https://example.com/")
    new = susbot.Analysis(GPTBOT_BLOCKED, site_url="https://example.com/")
    d = susbot.diff(old, new)
    assert d["is_changed"]
    assert any(b["name"] == "GPTBot" and b["new_verdict"] == "blocked" for b in d["bot_changes"])
    assert "example.com" in susbot.diff_markdown(old, new, "example.com")
    assert not susbot.diff(old, old)["is_changed"]


def test_cli(tmp_path):
    robots = tmp_path / "robots.txt"
    robots.write_text(GPTBOT_BLOCKED, encoding="utf-8")
    out = subprocess.run([sys.executable, "-m", "susbot", str(robots), "--format", "json"], capture_output=True, encoding="utf-8")
    assert out.returncode == 0, out.stderr
    assert json.loads(out.stdout)["summary"]["aiTraining"]["blocked"] >= 1
    assert susbot.main([str(robots), "--format", "summary", "--out", str(tmp_path / "s.txt")]) == 0
    assert (tmp_path / "s.txt").read_text(encoding="utf-8")
    bad = subprocess.run([sys.executable, "-m", "susbot", "--bogus"], capture_output=True, encoding="utf-8")
    assert bad.returncode == 2 and "--bogus" in bad.stderr
    fail = subprocess.run([sys.executable, "-m", "susbot", str(KITCHEN_SINK), "--fail-on", "error", "--format", "summary"], capture_output=True, encoding="utf-8")
    assert fail.returncode == 1
