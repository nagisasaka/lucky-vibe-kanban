import importlib.util
import os
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("acme_runner", Path(__file__).with_name("acme.py"))
acme = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acme)


class Certificates(unittest.TestCase):
    def test_validation_and_certbot_arguments(self):
        env = {"LVK_HOST": "8.8.8.8", "LVK_ACME_AGREE_TOS": "yes"}
        command = acme.certbot_command(env)
        self.assertIn("--ip-address", command)
        self.assertIn("shortlived", command)
        self.assertNotIn("--force-renewal", command)
        self.assertIn("--domains", acme.certbot_command({**env, "LVK_HOST": "lvk.example.com"}))
        self.assertIn("--ip-address", acme.certbot_command({**env, "LVK_HOST": "2606:4700:4700::1111"}))
        for host in ["", "127.0.0.1", "10.0.0.1", "192.168.1.2", "::1", "https://a.test", "a.test:443", "--staging", "a;id.test"]:
            with self.subTest(host=host), self.assertRaises(ValueError):
                acme.certbot_command({**env, "LVK_HOST": host})
        with self.assertRaises(ValueError):
            acme.certbot_command({**env, "LVK_ACME_AGREE_TOS": ""})
        with self.assertRaises(ValueError):
            acme.certbot_command({**env, "LVK_PREVIEW_DOMAIN": "preview.example.com"})

    def test_shared_route_grammar(self):
        cases = json.loads(Path(__file__).with_name("preview-routes.cases.json").read_text())
        for case in cases:
            with self.subTest(case=case):
                if case["valid"]:
                    self.assertEqual(acme.preview_routes(case["env"], case["env"]["LVK_HOST"]), case["routes"])
                else:
                    with self.assertRaises(ValueError):
                        acme.preview_routes(case["env"], case["env"]["LVK_HOST"])

    def test_route_orders_and_changed_hosts(self):
        env = {"LVK_HOST": "lvk.example.test", "LVK_ACME_AGREE_TOS": "yes"}
        for count in [0, 6, 24]:
            csv = ",".join(f"p{i}.example.test:4020" for i in range(count))
            command = acme.certbot_command({**env, "LVK_PREVIEW_ROUTES": csv})
            self.assertEqual(command.count("--domains"), count + 1)
            self.assertIn("--renew-with-new-domains", command)
        with self.assertRaisesRegex(ValueError, "25 identifiers.*externally managed"):
            acme.certbot_command({**env, "LVK_PREVIEW_ROUTES": csv + ",extra.test:4020"})
        for csv in ["dev.example.test:4020", "new.example.test:18443", ""]:
            command = acme.certbot_command({**env, "LVK_PREVIEW_ROUTES": csv})
            self.assertEqual([command[i + 1] for i, arg in enumerate(command) if arg == "--domains"],
                             [env["LVK_HOST"]] + ([csv.split(":")[0]] if csv else []))
        command = acme.certbot_command({**env, "LVK_HOST": "8.8.8.8", "LVK_PREVIEW_ROUTES": "dev.example.test:4020"})
        self.assertIn("--ip-address", command)
        self.assertIn("dev.example.test", command)

    def make_certificate(self, directory, serial):
        directory.mkdir()
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                        "-subj", "/CN=ip-test", "-addext", "subjectAltName=IP:192.0.2.1",
                        "-set_serial", str(serial), "-keyout", str(directory / "privkey.pem"),
                        "-out", str(directory / "fullchain.pem")], check=True, capture_output=True)

    def test_atomic_pair_export_and_recovery(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(acme.os, "chown"):
            root = Path(temporary)
            first, second, target = root / "first", root / "second", root / "tls"
            self.make_certificate(first, 1)
            self.make_certificate(second, 2)
            acme.publish(first, target)
            old = (target / "current").resolve()
            self.assertEqual((target / "privkey.pem").stat().st_mode & 0o777, 0o640)
            self.assertEqual((target / "fullchain.pem").read_bytes(), (first / "fullchain.pem").read_bytes())
            acme.publish(second, target)
            self.assertNotEqual((target / "current").resolve(), old)
            self.assertTrue(old.is_dir())
            self.assertEqual((target / "privkey.pem").read_bytes(), (second / "privkey.pem").read_bytes())
            # A repeated export repairs missing public links without reissuing.
            (target / "fullchain.pem").unlink()
            acme.publish(second, target)
            self.assertEqual((target / "fullchain.pem").read_bytes(), (second / "fullchain.pem").read_bytes())
            pointer = os.readlink(target / "current")
            (second / "privkey.pem").write_bytes((first / "privkey.pem").read_bytes())
            with self.assertRaises(Exception):
                acme.publish(second, target)
            self.assertEqual(os.readlink(target / "current"), pointer)

    def test_existing_manual_certificates_are_preserved(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(acme.os, "chown"):
            root = Path(temporary)
            self.make_certificate(root / "source", 1)
            (root / "tls").mkdir()
            (root / "tls/fullchain.pem").write_text("manual")
            with self.assertRaisesRegex(ValueError, "manually managed"):
                acme.publish(root / "source", root / "tls")
            self.assertEqual((root / "tls/fullchain.pem").read_text(), "manual")


if __name__ == "__main__":
    unittest.main()
