"""sus.bot robots.txt checker: Python bindings for the Rust engine.

    >>> import susbot
    >>> a = susbot.Analysis("User-agent: GPTBot\\nDisallow: /\\n", site_url="https://example.com/")
    >>> a.allowed("GPTBot", "/page")
    False
    >>> a.report["summary"]["aiTraining"]
    {'blocked': 1, 'total': 11}

The same engine runs on https://sus.bot/, in the ``susbot`` command (installed
with this package) and in the GitHub Action. See https://sus.bot/ for the
report schema.
"""

from __future__ import annotations

import json
from functools import cached_property
from typing import Any, Mapping, Sequence

from . import _susbot

__version__: str = _susbot.__version__

__all__ = ["Analysis", "diff", "diff_markdown", "default_config", "validate_config", "main", "__version__"]


class Analysis:
    """One robots.txt file, parsed, matched, linted and summarised.

    Args:
        text: The robots.txt content.
        site_url: Origin the file was served from; enables the checks that
            depend on it (sitemap hosts, absolute URLs, the report filename).
        config: TOML merged over the default configuration (see
            :func:`default_config`). Arrays replace the default ones.
        lang: Language code of ``locale``, recorded in the report.
        locale: A locale dictionary (``locales/<code>.json`` from the
            repository) as a dict or JSON text. English when absent.
        now: ISO 8601 timestamp for ``generatedAt`` and date notes.
        schema_url: Absolute URL of the report schema, for ``$schema``.
        fetch: How the file was fetched (the ``FetchInfo`` object of the
            report schema), for redirect and cross-host findings.

    Raises:
        ValueError: The configuration or locale does not load.
    """

    __slots__ = ("_inner", "__dict__")

    def __init__(
        self,
        text: str,
        *,
        site_url: str | None = None,
        config: str | None = None,
        lang: str | None = None,
        locale: Mapping[str, Any] | str | None = None,
        now: str | None = None,
        schema_url: str | None = None,
        fetch: Mapping[str, Any] | None = None,
    ) -> None:
        options: dict[str, Any] = {}
        if site_url is not None:
            options["siteUrl"] = site_url
        if config is not None:
            options["config"] = config
        if lang is not None:
            options["lang"] = lang
        if locale is not None:
            options["locale"] = locale if isinstance(locale, str) else json.dumps(locale)
        if now is not None:
            options["now"] = now
        if schema_url is not None:
            options["schemaUrl"] = schema_url
        if fetch is not None:
            options["fetch"] = dict(fetch)
        self._inner = _susbot.Analysis(text, json.dumps(options))

    @cached_property
    def report(self) -> dict[str, Any]:
        """The full report, as described by the published JSON schema."""
        return json.loads(self._inner.report_json(False))

    def report_json(self, *, pretty: bool = False) -> str:
        """The report as JSON text."""
        return self._inner.report_json(pretty)

    @property
    def text(self) -> str:
        """The analysed text."""
        return self._inner.text()

    @property
    def issues(self) -> list[dict[str, Any]]:
        """Lint findings: ``{level, kind, id, line, message}``."""
        return self.report["issues"]

    @property
    def crawlers(self) -> list[dict[str, Any]]:
        """Per-crawler verdicts (``open``, ``partial``, ``blocked``)."""
        return self.report["crawlers"]

    @property
    def security(self) -> list[dict[str, Any]]:
        """Disallowed paths that look sensitive."""
        return self.report["security"]

    def check_access(self, user_agent: str | Sequence[str], path: str) -> dict[str, Any]:
        """Whether a crawler may fetch ``path``, with the deciding rule.

        ``user_agent`` is one product token or several, most specific first.
        """
        tokens = [user_agent] if isinstance(user_agent, str) else list(user_agent)
        return json.loads(self._inner.check_access(tokens, path))

    def allowed(self, user_agent: str | Sequence[str], path: str) -> bool:
        """Shorthand for ``check_access(...)["allowed"]``."""
        return bool(self.check_access(user_agent, path)["allowed"])

    def clean_params(self, path: str) -> dict[str, Any]:
        """``path`` after the file's Yandex ``Clean-param`` rules."""
        return json.loads(self._inner.clean_params(path))

    def markdown(self) -> str:
        """The client audit as Markdown."""
        return self._inner.markdown()

    def html(self) -> str:
        """The client audit as a standalone HTML document."""
        return self._inner.html()

    def recommended_actions(self) -> list[str]:
        """The audit's recommended actions, most important first."""
        return self._inner.recommended_actions()

    def csv_tabs(self) -> dict[str, str]:
        """CSV text per tab id (see :meth:`tab_labels`)."""
        return dict(self._inner.csv_tabs())

    def tab_labels(self) -> dict[str, str]:
        """Translated label per CSV tab id."""
        return dict(self._inner.tab_labels())

    def tagged_csv(self) -> str:
        """Every tab in one CSV, each row tagged with its tab."""
        return self._inner.tagged_csv()

    def tsv(self, tab: str) -> str | None:
        """One tab as TSV, or None for an unknown tab id."""
        return self._inner.tsv(tab)

    def filename(self) -> str:
        """Suggested base filename for exports of this report."""
        return self._inner.filename()

    def crawler_list(self) -> list[dict[str, Any]]:
        """The crawler list of the active configuration."""
        return json.loads(self._inner.crawlers_json())

    def __repr__(self) -> str:
        s = self.report["summary"]
        return f"<susbot.Analysis groups={s['groups']} rules={s['rules']} issues={s['issues']} security={s['securityFindings']}>"


def diff(old: Analysis, new: Analysis) -> dict[str, Any]:
    """Semantic diff of two analyses built with the same options: crawler
    verdict flips, sensitive paths that appeared or went away, issues,
    sitemaps, and a unified text diff."""
    return json.loads(_susbot.diff_json(old._inner, new._inner))


def diff_markdown(old: Analysis, new: Analysis, domain: str) -> str:
    """The semantic diff as Markdown, headed with ``domain``."""
    return _susbot.diff_markdown(old._inner, new._inner, domain)


def default_config() -> str:
    """The embedded default configuration, as TOML text."""
    return _susbot.default_config()


def validate_config(toml: str) -> None:
    """Raise ValueError when ``toml`` does not merge over the defaults."""
    _susbot.validate_config(toml)


def main(argv: Sequence[str] | None = None) -> int:
    """The ``susbot`` command. ``argv`` excludes the program name; the
    process arguments are used when it is None."""
    from .__main__ import main as _main

    return _main(argv)
