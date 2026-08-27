#!/usr/bin/env python3
"""Execute Phase 33 GD-1..GD-5, including the real local-Mac pipeline."""
from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = ROOT / "target/debug/aetherctl"


def checked(command: list[str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def cli(*args: str, expect_success: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run([str(BIN), *args], cwd=ROOT, text=True, capture_output=True)
    if expect_success and result.returncode != 0:
        raise SystemExit(f"aetherctl failed: {args}\n{result.stdout}\n{result.stderr}")
    if not expect_success and result.returncode == 0:
        raise SystemExit(f"aetherctl unexpectedly accepted: {args}")
    return result


checked(["cargo", "build", "-q", "-p", "aetherctl"])

profile_proofs = checked(
    [
        "cargo",
        "test",
        "-q",
        "-p",
        "aethercore-security-audit",
        "--test",
        "compliance_red",
        "--",
        "--nocapture",
    ]
)
for line in profile_proofs.stdout.splitlines():
    marker = line.find("GD-")
    if marker >= 0:
        print(line[marker:])

signature_proofs = checked(
    [
        "cargo",
        "test",
        "-q",
        "-p",
        "aetherctl",
        "phase33_tests::gd2_and_gd3_report_determinism_signature_tamper_and_unsigned_honesty",
        "--",
        "--nocapture",
    ]
)
for line in signature_proofs.stdout.splitlines():
    marker = line.find("GD-")
    if marker >= 0:
        print(line[marker:])

with tempfile.TemporaryDirectory(prefix="aethercore-p33-live-") as temp_name:
    temp = pathlib.Path(temp_name)
    fixture = temp / "sshd_config"
    fixture.write_text(
        "PermitRootLogin yes\nPasswordAuthentication yes\nPubkeyAuthentication yes\n",
        encoding="utf-8",
    )

    # Independent CLI determinism proof with only timestamp allowlisted.
    first_json = temp / "determinism-first.json"
    second_json = temp / "determinism-second.json"
    cli(
        "sec",
        "audit",
        "--profile",
        "cis-l1",
        "--ssh",
        str(fixture),
        "--out",
        str(first_json),
        "--format",
        "both",
    )
    cli(
        "sec",
        "audit",
        "--profile",
        "cis-l1",
        "--ssh",
        str(fixture),
        "--out",
        str(second_json),
        "--format",
        "both",
    )
    first = json.loads(first_json.read_text(encoding="utf-8"))
    second = json.loads(second_json.read_text(encoding="utf-8"))
    if first["digest"] != second["digest"]:
        raise SystemExit("GD-2 digest drift")
    first["generated_unix_ms"] = 0
    second["generated_unix_ms"] = 0
    if first != second:
        raise SystemExit("GD-2 non-timestamp JSON drift")
    first_html = first_json.with_suffix(".html").read_bytes()
    second_html = second_json.with_suffix(".html").read_bytes()
    if first_html != second_html:
        raise SystemExit("GD-2 HTML drift")
    print("GD-2 CLI JSON identical after generated_unix_ms allowlist normalization: true")
    print("GD-2 CLI HTML byte-identical: true")

    # Real host, explicit temporary owner key, signed JSON + standalone HTML.
    owner_seed = temp / "owner.seed"
    cli("--output", "json", "keys", "generate", "--out", str(owner_seed))
    live_json = temp / "live.json"
    cli(
        "--output",
        "json",
        "sec",
        "audit",
        "--profile",
        "cis-l1",
        "--out",
        str(live_json),
        "--format",
        "both",
        "--sign",
        "--key",
        str(owner_seed),
    )
    verified = cli("--output", "json", "compliance", "verify", str(live_json))
    live = json.loads(live_json.read_text(encoding="utf-8"))
    if not live.get("signed") or not live.get("signature"):
        raise SystemExit("GD-4 live report was not signed")

    live_html = live_json.with_suffix(".html")
    html_bytes = live_html.read_bytes()
    html = html_bytes.decode("utf-8")
    external_urls = html.count("http://") + html.count("https://")
    if not html_bytes or external_urls != 0:
        raise SystemExit("GD-4 HTML standalone proof failed")

    print("GD-4 LIVE MAC SCORE BLOCK")
    print(json.dumps(live["score"], indent=2, ensure_ascii=False))
    print(
        "GD-4 SIGNED OFFLINE VERIFY",
        json.dumps(
            {
                "exit": verified.returncode,
                "signed": live["signed"],
                "digest": live["digest"],
            },
            sort_keys=True,
        ),
    )
    print(
        "GD-4 HTML AIR-GAP",
        json.dumps(
            {
                "path": str(live_html),
                "bytes": len(html_bytes),
                "external_http_https_occurrences": external_urls,
                "standalone_local_file": live_html.is_file(),
            },
            sort_keys=True,
        ),
    )

    # Flip one ASCII byte inside signature_hex while preserving valid JSON.
    signature_tampered = json.loads(live_json.read_text(encoding="utf-8"))
    old_signature = signature_tampered["signature"]["signature_hex"]
    flipped = "0" if old_signature[0] != "0" else "1"
    signature_tampered["signature"]["signature_hex"] = flipped + old_signature[1:]
    tampered_path = temp / "live-signature-tampered.json"
    tampered_path.write_text(json.dumps(signature_tampered, indent=2), encoding="utf-8")
    tamper_result = cli(
        "--output", "json", "compliance", "verify", str(tampered_path), expect_success=False
    )
    tamper_output = (tamper_result.stdout + tamper_result.stderr).strip()
    if "sec.signatureMismatch" not in tamper_output and "signature mismatch" not in tamper_output:
        raise SystemExit(f"GD-3 signature tamper was not typed: {tamper_output}")
    print(
        "GD-3 ONE-BYTE SIGNATURE TAMPER",
        json.dumps({"exit": tamper_result.returncode, "typed_error": tamper_output}),
    )

    # Unsigned report remains honest and still verifies its deterministic integrity.
    unsigned_verified = cli("--output", "json", "compliance", "verify", str(first_json))
    if first.get("signed") or "signature" in first:
        raise SystemExit("GD-3 unsigned honesty failed")
    print(
        "GD-3 UNSIGNED HONESTY",
        json.dumps(
            {
                "exit": unsigned_verified.returncode,
                "signed": False,
                "signature_present": False,
            },
            sort_keys=True,
        ),
    )
