#!/usr/bin/env python3
"""Serve the live console and run fixture queries against a local demo."""

from argparse import Namespace
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from threading import Lock

from run_proof import FIXTURE_END_BLOCK, ROOT, TOKEN, await_certificate, read_deployment, submit_query
from verify_step4 import read_json, send, wait_result

HOST = "127.0.0.1"
PORT = 8765
ORIGIN = f"http://{HOST}:{PORT}"
GATE_BLOCK = 1000
ALICE = "0x" + "aa" * 20
ZERO_ADDRESS = "0x" + "00" * 20
GATES = {
    "airdrop": (8080, "CONSUMER_CONTRACT"),
    "governance": (8082, "GOVERNANCE_CONTRACT"),
}
ASSETS = {
    "/": ("live/index.html", "text/html; charset=utf-8"),
    "/styles.css": ("live/styles.css", "text/css; charset=utf-8"),
    "/app.js": ("live/app.js", "text/javascript; charset=utf-8"),
    "/evidence": ("holderproof_vericredit_idx_console.html", "text/html; charset=utf-8"),
}
QUERY_LOCK = Lock()
DEVNET_ERROR = "The local demo could not complete the request. Check the devnet terminal and try again."
BACKEND_ERRORS = (OSError, KeyError, ValueError, RuntimeError, subprocess.SubprocessError)


def matches_certificate(result: dict, certificate: dict) -> bool:
    return (
        certificate["status"] == "Finalized"
        and result["queryHash"] == certificate["query_hash"]
        and result["balance"] == certificate["balance"]
        and result["block"] == certificate["block_number"]
    )


def snapshot() -> dict:
    certificate = read_json(8081, "/api/certificate")
    gates = {}
    if certificate["block_number"] == GATE_BLOCK:
        for name, (port, _) in GATES.items():
            result = read_json(port)
            if matches_certificate(result, certificate):
                gates[name] = result
    return {"certificate": certificate, "gates": gates, "gateBlock": GATE_BLOCK}


def run_query(holder: str, block: int) -> dict:
    deployment = read_deployment()
    query = Namespace(holder=holder, block=block, token=TOKEN, endpoint="ws://127.0.0.1:10087/ws")
    indexer = deployment["INDEXER_CONTRACT"]
    submit_query(indexer, query)
    certificate = await_certificate(indexer, query)
    if certificate["holder"].lower() != holder or certificate["block_number"] != block:
        raise RuntimeError("Certificate does not match the requested holder and block")

    gates = {}
    if block != GATE_BLOCK:
        return {"certificate": certificate, "gates": gates, "gateBlock": GATE_BLOCK}

    for name, (port, contract_key) in GATES.items():
        policy = read_json(port)
        if policy["block"] != GATE_BLOCK or policy["holder"] != ZERO_ADDRESS:
            raise RuntimeError("Web demo gate policy changed")
        previous_count = policy["checkCount"]
        send(deployment[contract_key], "check_eligible_for(address)", holder)
        gates[name] = wait_result(
            port,
            lambda result: result["checkCount"] > previous_count
            and matches_certificate(result, certificate),
        )
    return {"certificate": certificate, "gates": gates, "gateBlock": GATE_BLOCK}


def prepare_demo_gates() -> None:
    deployment = read_deployment()
    airdrop = read_json(8080)
    governance = read_json(8082)
    indexer = airdrop["indexer"]
    send(
        deployment["CONSUMER_CONTRACT"], "configure_policy(address,uint64,uint256)",
        indexer, GATE_BLOCK, airdrop["threshold"],
    )
    send(
        deployment["GOVERNANCE_CONTRACT"], "configure_policy(address,uint64,uint256,uint64)",
        indexer, GATE_BLOCK, governance["threshold"], governance["proposalId"],
    )
    for port, _ in GATES.values():
        wait_result(port, lambda result: result["holder"] == ZERO_ADDRESS and result["block"] == GATE_BLOCK)
    run_query(ALICE, GATE_BLOCK)


class WebHandler(BaseHTTPRequestHandler):
    def send_content(self, status: HTTPStatus, content_type: str, body: bytes) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)

    def send_json(self, status: HTTPStatus, data: dict) -> None:
        self.send_content(status, "application/json", json.dumps(data).encode("utf-8"))

    def local_request(self) -> bool:
        if self.headers.get("Host") == f"{HOST}:{PORT}" and self.headers.get("Origin", ORIGIN) == ORIGIN:
            return True
        self.send_json(HTTPStatus.FORBIDDEN, {"error": "Use the local console address."})
        return False

    def do_GET(self) -> None:
        if not self.local_request():
            return
        if self.path in ASSETS:
            filename, content_type = ASSETS[self.path]
            self.send_content(HTTPStatus.OK, content_type, (ROOT / "ui" / filename).read_bytes())
        elif self.path == "/api/snapshot":
            try:
                result = snapshot()
            except BACKEND_ERRORS:
                self.send_json(HTTPStatus.SERVICE_UNAVAILABLE, {"error": DEVNET_ERROR})
            else:
                self.send_json(HTTPStatus.OK, result)
        else:
            self.send_json(HTTPStatus.NOT_FOUND, {"error": "Page not found."})

    def read_query(self) -> tuple[str, int]:
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if self.headers.get_content_type() != "application/json" or not 0 < size <= 256:
                raise ValueError
            request = json.loads(self.rfile.read(size))
            if not isinstance(request, dict):
                raise ValueError
        except ValueError:
            raise ValueError("Send the query as a JSON object.") from None

        holder = request.get("holder")
        if not isinstance(holder, str) or not re.fullmatch(r"0x[0-9a-fA-F]{40}", holder) or holder == ZERO_ADDRESS:
            raise ValueError("Enter a nonzero 20-byte holder address.")
        block = request.get("block", GATE_BLOCK)
        if type(block) is not int or not 0 <= block <= FIXTURE_END_BLOCK:
            raise ValueError(f"Enter a whole snapshot block from 0 to {FIXTURE_END_BLOCK}.")
        return holder.lower(), block

    def do_POST(self) -> None:
        if not self.local_request():
            return
        if self.path != "/api/query":
            self.send_json(HTTPStatus.NOT_FOUND, {"error": "Page not found."})
            return
        try:
            holder, block = self.read_query()
        except ValueError as error:
            self.send_json(HTTPStatus.BAD_REQUEST, {"error": str(error)})
            return
        if not QUERY_LOCK.acquire(blocking=False):
            self.send_json(HTTPStatus.CONFLICT, {"error": "A query is already running. Try again shortly."})
            return
        try:
            result = run_query(holder, block)
        except BACKEND_ERRORS as error:
            self.log_error("Devnet query failed (%s)", type(error).__name__)
            self.send_json(HTTPStatus.SERVICE_UNAVAILABLE, {"error": DEVNET_ERROR})
        else:
            self.send_json(HTTPStatus.OK, result)
        finally:
            QUERY_LOCK.release()


def main() -> int:
    os.environ["PATH"] = f"{Path.home() / '.shakenup/bin'}:{os.environ.get('PATH', '')}"
    try:
        with ThreadingHTTPServer((HOST, PORT), WebHandler) as server:
            prepare_demo_gates()
            print(f"HolderProof live console: {ORIGIN}", flush=True)
            server.serve_forever()
    except KeyboardInterrupt:
        pass
    except BACKEND_ERRORS as error:
        print(f"Could not start the HolderProof console ({type(error).__name__}). Check the demo and web port.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
