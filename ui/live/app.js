"use strict";

const byId = (id) => document.getElementById(id);
const form = byId("query-form");
const holder = byId("holder");
const blockInput = byId("block");
const runButton = byId("run-query");
const numberFormat = new Intl.NumberFormat("en");
let displayedCertificate = null;
let gateBlock = Number(blockInput.defaultValue);

function formatUnits(value) {
  return value == null ? "—" : numberFormat.format(BigInt(value));
}

function updateHolderAddress() {
  byId("holder-address").textContent = holder.value;
}

function setConnection(state, message) {
  byId("connection").dataset.state = state;
  byId("connection-label").textContent = message;
}

function setMessage(message, error = false) {
  const status = byId("query-status");
  status.textContent = message;
  status.dataset.error = String(error);
}

function setPill(id, label, tone) {
  const pill = byId(id);
  pill.textContent = label;
  pill.dataset.tone = tone;
}

function renderGate(name, result, isGateBlock) {
  const unavailable = isGateBlock ? "Waiting" : "Not evaluated";
  const status = result ? (result.eligible ? "Eligible" : "Ineligible") : unavailable;
  const tone = result ? (result.eligible ? "positive" : "negative") : "neutral";
  setPill(`${name}-status`, status, tone);
  byId(`${name}-threshold`).textContent = formatUnits(result?.threshold);
}

function renderResult({ certificate, gates, gateBlock: eligibilityBlock }) {
  gateBlock = eligibilityBlock;
  displayedCertificate = certificate;
  const option = [...holder.options].find(({ value }) => value === certificate.holder);
  const address = certificate.holder;
  byId("result-holder").textContent = option?.label || `${address.slice(0, 6)}…${address.slice(-4)}`;
  byId("result-holder").title = address;
  byId("balance").textContent = formatUnits(certificate.balance);
  byId("result-block").textContent = formatUnits(certificate.block_number);
  byId("result-signers").textContent = `${new Set(certificate.signer_ids).size} of 4`;
  byId("result-events").textContent = formatUnits(certificate.checkpoint.event_count);
  const tone = certificate.status === "Finalized" ? "positive" : "negative";
  setPill("certificate-status", certificate.status, tone);
  byId("query-hash").textContent = certificate.query_hash;
  byId("event-root").textContent = certificate.checkpoint.event_merkle_root;
  byId("certificate-details").hidden = false;

  const isGateBlock = certificate.block_number === gateBlock;
  renderGate("airdrop", gates.airdrop, isGateBlock);
  renderGate("governance", gates.governance, isGateBlock);
  const sharedProof = byId("shared-proof");
  if (!isGateBlock) {
    sharedProof.textContent = `Gate decisions use block ${formatUnits(gateBlock)}.`;
  } else if (gates.airdrop && gates.governance) {
    sharedProof.textContent = "Both gates used this query hash.";
  } else {
    sharedProof.textContent = "Run the query to check both gates.";
  }
}

async function request(path, options) {
  const response = await fetch(path, options);
  const data = await response.json();
  if (!response.ok) throw new Error(data.error || "The request failed.");
  return data;
}

function setBusy(busy, label = "Run live query") {
  holder.disabled = busy;
  blockInput.disabled = busy;
  runButton.disabled = busy;
  runButton.textContent = label;
  byId("results").setAttribute("aria-busy", String(busy));
}

async function connect() {
  setBusy(true, "Connecting…");
  try {
    const result = await request("/api/snapshot");
    setConnection("ready", "Local devnet ready");
    renderResult(result);
    blockInput.value = String(result.certificate.block_number);
    if ([...holder.options].some(({ value }) => value === result.certificate.holder)) {
      holder.value = result.certificate.holder;
      updateHolderAddress();
    }
  } catch (error) {
    setConnection("offline", "Devnet unavailable");
    setMessage(error.message, true);
  } finally {
    setBusy(false);
  }
}

holder.addEventListener("change", updateHolderAddress);
form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const block = blockInput.valueAsNumber;
  setBusy(true, "Running query…");
  setMessage(block === gateBlock
    ? "Requesting the certificate and evaluating both gates."
    : "Requesting the certificate for this historical block.");
  try {
    const result = await request("/api/query", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ holder: holder.value, block }),
    });
    renderResult(result);
    setConnection("ready", "Local devnet ready");
    setMessage("Query complete.");
  } catch (error) {
    setMessage(error.message, true);
  } finally {
    setBusy(false);
  }
});

byId("save-certificate").addEventListener("click", () => {
  if (!displayedCertificate) return;
  const blob = new Blob([JSON.stringify(displayedCertificate, null, 2) + "\n"], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `holderproof-${displayedCertificate.query_hash.slice(0, 12)}.json`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});

updateHolderAddress();
connect();
