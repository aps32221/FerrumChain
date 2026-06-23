"""Quickstart against a local dev node:  ./target/release/ferrum-node --dev

    pip install -e .
    python examples/quickstart.py

Accredits Alice as an issuer (via sudo), anchors a DID, files a tax obligation.
"""
import hashlib

from ferrum_sdk import FerrumClient


def blake2_256(data: str) -> str:
    return "0x" + hashlib.blake2b(data.encode(), digest_size=32).hexdigest()


def main():
    ferrum = FerrumClient.connect("ws://127.0.0.1:9944")
    alice = ferrum.keypair("//Alice")

    subject = {"chain_tag": "tw", "id": "citizen-0001"}

    # 1. Governance accredits Alice as an issuer (sudo wraps governance on --dev).
    inner = ferrum.identity.register_issuer(alice.ss58_address)
    sudo = ferrum.substrate.compose_call(call_module="Sudo", call_function="sudo", call_params={"call": inner})
    ferrum.sign_and_send(sudo, alice)
    print("issuer accredited")

    # 2. Anchor the DID — doc_hash is a commitment, never the document itself.
    anchor = ferrum.identity.anchor_did({
        "did": subject,
        "controller": alice.ss58_address,
        "doc_hash": blake2_256("off-chain DID document for citizen #1"),
        "keys": [{"kind": "Sr25519", "key_hash": blake2_256("device-key")}],
        "revocation_commitment": blake2_256("rev-acc-0"),
        "anchored_at": 0,
    })
    ferrum.sign_and_send(anchor, alice)
    print("DID anchored")

    # 3. File a fiat-denominated tax obligation.
    obligation = ferrum.tax.file_obligation({
        "subject": subject,
        "kind": "Income",
        "amount_due": {"currency": "TWD", "minor_units": 1234500},
        "detail_commitment": blake2_256("encrypted return detail"),
        "settled": False,
    })
    ferrum.sign_and_send(obligation, alice)
    print("obligation filed")

    # 4. Read it back.
    stored = ferrum.query("Tax", "Obligations", [(subject, 0)])
    print("obligation on-chain:", stored.value)

    ferrum.close()


if __name__ == "__main__":
    main()
