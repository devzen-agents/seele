"""Tests of the oracle with simulated outputs, NOT tests of Claude Code."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest

import markdown_edit_regression as regression


class MarkdownEditRegressionTests(unittest.TestCase):
    def test_fixtures_are_long_and_have_unique_sentinels(self):
        for eol in ("lf", "crlf"):
            with self.subTest(eol=eol):
                data = regression.fixture(eol)
                self.assertGreater(len(data), 200 * 1024)
                for marker in ("START", "MIDDLE", "END"):
                    self.assertEqual(data.count(f"<!-- SENTINEL:{marker} -->".encode()), 1)
                data.decode("utf-8")
                self.assertEqual(b"\r" in data, eol == "crlf")

    def test_only_exact_authorized_edit_passes(self):
        for eol in ("lf", "crlf"):
            for position in regression.POSITIONS:
                with self.subTest(eol=eol, position=position):
                    good = regression.expected(eol, position)
                    self.assertTrue(regression.verify(good, eol, position)["passed"])
                    bad_cases = {
                        "no_op": regression.fixture(eol),
                        "truncated": good[:len(good) // 2],
                        "lost_start": good[1:],
                        "lost_final_newline": good[:-1],
                        "extra": good + b"extra",
                        "unrelated_middle_change": good.replace(b"synthetic-1000", b"synthetic-9999"),
                        "unicode_changed": good.replace("café".encode(), b"cafe", 1),
                        "empty": b"",
                        "newlines_changed": good.replace(b"\r\n", b"\n") if eol == "crlf"
                        else good.replace(b"\n", b"\r\n"),
                    }
                    for corruption, data in bad_cases.items():
                        with self.subTest(corruption=corruption):
                            result = regression.verify(data, eol, position)
                            self.assertFalse(result["passed"])
                            self.assertIsNotNone(result["first_mismatch_byte"])

    def test_cli_prepare_verify_and_refuse_existing_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "synthetic run 日本語"
            args = [str(directory), "--eol", "crlf", "--position", "end"]
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(regression.main(["prepare"] + args), 0)
                path = directory / regression.FILENAME
                original = path.read_bytes()
                with self.assertRaises(SystemExit) as error:
                    regression.main(["prepare"] + args)
                self.assertEqual(error.exception.code, 2)
                self.assertEqual(path.read_bytes(), original)
                self.assertEqual(regression.main(["verify"] + args), 1)
                path.write_bytes(regression.expected("crlf", "end"))
                self.assertEqual(regression.main(["verify"] + args), 0)
                self.assertEqual(list(directory.iterdir()), [path])

    def test_missing_input_is_an_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                regression.main(["verify", temporary])
            self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
