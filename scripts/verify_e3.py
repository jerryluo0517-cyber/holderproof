#!/usr/bin/env python3
"""Run certificate and gate regressions against a fresh local demo."""

import json
import shlex
import subprocess
import time

from verify_step4 import ROOT, read_json, require, wait_result

RUN_DIR = ROOT / ".run/step4"
EVIDENCE = ROOT / ".run/acceptance/e3-results.json"
RPC = "http://127.0.0.1:8545"
BLOCK = 1000
GATES = (
    (8080, "CONSUMER_CONTRACT", "claim()", "claimCount"),
    (8082, "GOVERNANCE_CONTRACT", "vote()", "voteCount"),
)


def wait_for(read, predicate, label):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        value = read()
        if predicate(value):
            return value
        time.sleep(0.25)
    raise RuntimeError(f"Timed out waiting for {label}")


def cast(*arguments):
    result = subprocess.run(
        ["cast", *map(str, arguments), "--rpc-url", RPC],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return json.loads(result.stdout)


def submit(sender, contract, method, *arguments):
    receipt = cast("send", "--json", "--unlocked", "--from", sender, contract, method, *arguments)
    require(int(receipt["status"], 16) == 1, f"Sequencer rejected {method}")
    # Anvil success means the request was sequenced, not that a Lyquid committed it.
    return receipt["transactionHash"]


def contract_addresses():
    state = {}
    for line in (RUN_DIR / "state.env").read_text().splitlines():
        key, value = line.split("=", 1)
        state[key] = shlex.split(value)[0] if value else ""
    return state


def certify(holder, token):
    output = subprocess.check_output(
        ["bash", str(ROOT / "scripts/run-proof.sh"), "--token", token,
         "--holder", holder, "--block", str(BLOCK)],
        text=True,
        timeout=90,
    )
    certificate = json.loads(output)
    require(certificate["holder"].lower() == holder.lower(), "Wrong holder in certificate")
    return certificate


def configure(port, contract, sender, indexer, holder, threshold, proposal=1):
    arguments = [indexer, holder, BLOCK, threshold]
    signature = "configure(address,address,uint64,uint256)"
    if port == 8082:
        signature = "configure(address,address,uint64,uint256,uint64)"
        arguments.append(proposal)
    return submit(sender, contract, signature, *arguments)


def wait_configuration(port, holder, threshold, proposal):
    return wait_result(port, lambda result: (
        result["holder"].lower() == holder.lower()
        and int(result["threshold"]) == threshold
        and result["checkCount"] == 0
        and (port != 8082 or result["proposalId"] == proposal)
    ))


def check_policy(port, contract, owner, expected):
    before = read_json(port)["checkCount"]
    submit(owner, contract, "check_eligible()")
    result = wait_result(port, lambda value: value["checkCount"] == before + 1)
    require(result["eligible"] is expected, f"Unexpected gate decision on port {port}")
    return result


def main():
    require(int(cast("rpc", "eth_chainId"), 16) == 31337, "Run E3 only against the local Anvil demo")
    owner, holder, revoked_holder = cast("rpc", "eth_accounts")[:3]
    state = contract_addresses()
    demo = read_json(8080)
    baseline = read_json(8081, "/api/certificate")
    require(baseline["status"] == "Finalized" and int(baseline["balance"]) > 0,
            "Start with a fresh demo via make e3")
    require(all(read_json(port)[counter] == 0 for port, _, _, counter in GATES),
            "Start with fresh gates via make e3")
    token = baseline["token"]
    indexer_target = demo["indexer"]
    EVIDENCE.unlink(missing_ok=True)
    evidence = []

    certificate = certify(holder, token)
    require(certificate["balance"] == "0", "Expected zero balance for an absent holder")
    evidence.append({"case": "absent holder", "balance": certificate["balance"]})
    for port, address_key, method, counter in GATES:
        contract = state[address_key]

        configure(port, contract, owner, indexer_target, holder, 1)
        wait_configuration(port, holder, 1, 1)
        too_low = submit(holder, contract, method)
        result = check_policy(port, contract, owner, False)
        require(result[counter] == 0, f"{method} recorded an ineligible action")

        configure(port, contract, owner, indexer_target, holder, 0)
        wait_configuration(port, holder, 0, 1)
        unexpected_config = configure(port, contract, holder, indexer_target, holder, 1)
        wrong_holder = submit(owner, contract, method)
        result = check_policy(port, contract, owner, True)
        require(result["threshold"] == "0", "Non-initializer changed the gate policy")
        require(result[counter] == 0, f"{method} recorded another holder's action")

        accepted = submit(holder, contract, method)
        wait_result(port, lambda result: result[counter] == 1)
        duplicate = submit(holder, contract, method)
        result = check_policy(port, contract, owner, True)
        require(result[counter] == 1, f"{method} recorded a duplicate")
        evidence.append({
            "case": method,
            "below_threshold_tx": too_low,
            "different_holder_tx": wrong_holder,
            "non_initializer_configuration_tx": unexpected_config,
            "accepted_tx": accepted,
            "duplicate_tx": duplicate,
            "committed_actions": result[counter],
        })
        print(f"[PASS] {method} eligibility, holder binding, and one-time recording")

    revoked = certify(revoked_holder, token)
    for port, address_key, _, _ in GATES:
        contract = state[address_key]
        configure(port, contract, owner, indexer_target, revoked_holder, 0)
        wait_configuration(port, revoked_holder, 0, 1)
        check_policy(port, contract, owner, True)

    non_initializer_revocation = submit(holder, state["INDEXER_CONTRACT"],
                              "invalidate_certificate(address,uint64)", revoked_holder, BLOCK)
    for port, address_key, _, _ in GATES:
        check_policy(port, state[address_key], owner, True)
    require(read_json(8081, "/api/certificate") == revoked,
            "Non-initializer changed certificate status")

    revoke_tx = submit(owner, state["INDEXER_CONTRACT"],
                       "invalidate_certificate(address,uint64)", revoked_holder, BLOCK)
    invalidated = wait_for(
        lambda: read_json(8081, "/api/certificate"),
        lambda value: value["status"] == "Invalidated",
        "certificate invalidation",
    )
    require(invalidated == {**revoked, "status": "Invalidated"},
            "Revocation changed the signed fact")
    for port, address_key, method, counter in GATES:
        contract = state[address_key]
        blocked = submit(revoked_holder, contract, method)
        # A later committed configuration confirms the earlier action was processed.
        # Reconfiguration preserves the aggregate action count.
        configure(port, contract, owner, indexer_target, baseline["holder"], 1)
        result = wait_configuration(port, baseline["holder"], 1, 1)
        require(result[counter] == 1, f"{method} accepted a revoked certificate")
        evidence.append({"case": f"revoked {method}", "tx": blocked, "committed_actions": result[counter]})
    evidence.append({
        "case": "revocation",
        "non_initializer_tx": non_initializer_revocation,
        "initializer_tx": revoke_tx,
        "certificate": revoked["certificate_id"],
    })
    print("[PASS] Only the initializer can revoke. Revocation blocks both gate actions.")

    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(evidence, indent=2) + "\n")
    print(f"[PASS] Wrote {EVIDENCE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
