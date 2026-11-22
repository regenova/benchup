"""Offline checks for the release gate protecting already-installed updaters."""
import unittest
from io import BytesIO
from urllib.error import HTTPError
from unittest.mock import Mock, patch

import verify_update_transfer as migration


class TransferTests(unittest.TestCase):
    def fetch(self, old=None, new=None, old_releases=None, new_releases=None):
        repo = {"id": 1234, "full_name": migration.CURRENT, "private": False}
        return Mock(side_effect=[
            repo if old is None else old, repo if new is None else new,
            [{"id": 42}] if old_releases is None else old_releases,
            [{"id": 42}] if new_releases is None else new_releases,
        ])

    def test_transferred_repository_retains_numeric_identity_and_public_release_access(self):
        for source in [migration.LEGACY, migration.CURRENT, "BenchUp3D/app", "Ketchup3D/editor", "BenchUp3D/editor", "benchup3d/BENCHUP", "BenchUp3D/benchup", "REGENOVA/BENCHUP"]:
            fetch = self.fetch()
            self.assertIn("Verified", migration.verify(source, fetch))
            self.assertEqual(fetch.call_args_list[1].args[0],
                             migration.API + "regenova/benchup")
            self.assertEqual(fetch.call_args_list[3].args[0],
                             migration.API + "regenova/benchup/releases?per_page=10&page=1")
            self.assertEqual(fetch.call_args_list[2].args[0],
                             migration.API + migration.LEGACY + "/releases?per_page=10&page=1")
        self.assertIn("Verified", migration.verify(migration.CURRENT, self.fetch(old_releases=[], new_releases=[])))

    def test_recreated_old_repository_or_wrong_transfer_blocks_release(self):
        for old in [{"id": 999, "full_name": migration.LEGACY},
                    {"id": 1234, "full_name": migration.CURRENT, "private": True},
                    {"id": 1234, "full_name": migration.CURRENT},
                    {"id": 1234, "full_name": "someone/else"}, {}, []]:
            with self.subTest(old=old), self.assertRaisesRegex(ValueError, "same repository"):
                migration.verify(migration.CURRENT, self.fetch(old=old))

    def test_lost_release_redirect_invalid_metadata_and_different_releases_block_release(self):
        for old_releases in [[], {}, [{"id": 99}], [{}]]:
            with self.subTest(releases=old_releases), self.assertRaises(ValueError):
                migration.verify(migration.CURRENT, self.fetch(old_releases=old_releases))
        error = HTTPError("url", 404, "missing", {}, None)
        self.addCleanup(error.close)
        with self.assertRaises(HTTPError):
            migration.verify(migration.CURRENT, Mock(side_effect=error))

    def test_forks_do_not_depend_on_the_official_transfer(self):
        fetch = Mock()
        self.assertIn("Skipped", migration.verify("someone/editor", fetch))
        fetch.assert_not_called()
        with self.assertRaises(ValueError):
            migration.verify("../invalid/repository", fetch)

    def test_explicit_retirement_warns_and_still_validates_the_active_public_feed(self):
        missing = HTTPError("url", 404, "missing", {}, None)
        self.addCleanup(missing.close)
        repo = {"id": 1234, "full_name": migration.CURRENT, "private": False}
        fetch = Mock(side_effect=[missing, repo, [{"id": 42}]])
        result = migration.verify(migration.CURRENT, fetch, allow_missing_legacy=True)
        self.assertIn("WARNING", result)
        self.assertIn("manual update", result)
        self.assertEqual(fetch.call_args_list[2].args[0],
                         migration.API + migration.CURRENT + "/releases?per_page=10&page=1")
        for invalid_repo in [{}, [], dict(repo, private=True), dict(repo, full_name="someone/else")]:
            with self.subTest(repo=invalid_repo), self.assertRaises(ValueError):
                migration.verify(migration.CURRENT, Mock(side_effect=[missing, invalid_repo]),
                                 allow_missing_legacy=True)
        for invalid_releases in [{}, [{}]]:
            with self.subTest(releases=invalid_releases), self.assertRaises(ValueError):
                migration.verify(migration.CURRENT, Mock(side_effect=[missing, repo, invalid_releases]),
                                 allow_missing_legacy=True)

    def test_retirement_does_not_ignore_other_errors_or_recreated_legacy_repositories(self):
        for status in [403, 429, 500]:
            error = HTTPError("url", status, "failure", {}, None)
            self.addCleanup(error.close)
            with self.subTest(status=status), self.assertRaises(HTTPError):
                migration.verify(migration.CURRENT, Mock(side_effect=error), allow_missing_legacy=True)
        for old in [{"id": 999, "full_name": migration.LEGACY}, [], {}]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                migration.verify(migration.CURRENT, self.fetch(old=old), allow_missing_legacy=True)

    def test_authenticated_fallback_drafts_are_not_public_releases(self):
        fetch = self.fetch(old_releases=[{"id": 42}, {"id": 99, "draft": True}])
        self.assertIn("Verified", migration.verify(migration.CURRENT, fetch))

    @patch.dict("os.environ", {"GH_TOKEN": "test-workflow-token"}, clear=True)
    def test_rate_limit_retries_with_workflow_token(self):
        for status, headers in [(403, {"X-RateLimit-Remaining": "0"}), (429, {})]:
            error = HTTPError("url", status, "rate limited", headers, None)
            authorizations = []

            def open_request(request, timeout):
                authorizations.append(request.get_header("Authorization"))
                if len(authorizations) == 1:
                    raise error
                return BytesIO(b'{"id":1234}')

            with self.subTest(status=status), patch.object(
                migration.urllib.request, "urlopen", side_effect=open_request
            ):
                self.assertEqual(migration.fetch_json(migration.API + migration.LEGACY), {"id": 1234})
                self.assertEqual(authorizations, [None, "Bearer test-workflow-token"])

    def test_missing_redirects_and_rate_limits_without_token_still_fail(self):
        for status, headers, token in [
            (404, {}, "test-workflow-token"),
            (403, {}, "test-workflow-token"),
            (403, {"X-RateLimit-Remaining": "0"}, ""),
        ]:
            error = HTTPError("url", status, "blocked", headers, None)
            self.addCleanup(error.close)
            with self.subTest(status=status, token=bool(token)), patch.dict(
                "os.environ", {"GH_TOKEN": token}, clear=True
            ), patch.object(migration.urllib.request, "urlopen", side_effect=error) as fetch:
                with self.assertRaises(HTTPError):
                    migration.fetch_json(migration.API + migration.LEGACY)
                fetch.assert_called_once()
