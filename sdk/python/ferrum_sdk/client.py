"""FerrumClient — a typed thin wrapper over substrate-interface.

Each pallet namespace builds a `GenericCall` via `substrate.compose_call`; submit
it with `client.sign_and_send(call, keypair)`.
"""
from __future__ import annotations

from typing import Any, Mapping, Optional, Sequence

from substrateinterface import SubstrateInterface, Keypair

from . import helpers as H

DEFAULT_ENDPOINT = "ws://127.0.0.1:9944"


class _Ns:
    def __init__(self, substrate: SubstrateInterface, module: str):
        self._s = substrate
        self._m = module

    def _call(self, function: str, params: Mapping[str, Any]):
        return self._s.compose_call(call_module=self._m, call_function=function, call_params=dict(params))


class IdentityNs(_Ns):
    def anchor_did(self, doc: Mapping[str, Any]):
        return self._call("anchor_did", {"doc": H.did_document(doc)})

    def rotate_keys(self, did: Mapping[str, Any], keys: Sequence[Mapping[str, Any]]):
        return self._call("rotate_keys", {"did": H.did(did), "keys": [H.did_key_ref(k) for k in keys]})

    def update_revocation(self, commitment: H.Bytes32):
        return self._call("update_revocation", {"commitment": H.h32(commitment)})

    def register_issuer(self, who: str):
        return self._call("register_issuer", {"who": who})


class CredentialNs(_Ns):
    def issue(self, anchor: Mapping[str, Any]):
        return self._call("issue", {"anchor": H.credential_anchor(anchor)})

    def revoke(self, payload_hash: H.Bytes32):
        return self._call("revoke", {"payload_hash": H.h32(payload_hash)})

    def set_status(self, payload_hash: H.Bytes32, status: str):
        return self._call("set_status", {"payload_hash": H.h32(payload_hash), "status": status})

    def log_presentation(self, nullifier: H.Bytes32, commitment: H.Bytes32):
        return self._call("log_presentation", {"nullifier": H.h32(nullifier), "commitment": H.h32(commitment)})


class TaxNs(_Ns):
    def anchor_invoice(self, anchor: Mapping[str, Any]):
        return self._call("anchor_invoice", {"anchor": H.invoice_anchor(anchor)})

    def withhold(self, subject: Mapping[str, Any], kind: str, amount: Mapping[str, Any]):
        return self._call("withhold", {"subject": H.did(subject), "kind": kind, "amount": H.fiat_amount(amount)})

    def file_obligation(self, obligation: Mapping[str, Any]):
        return self._call("file_obligation", {"obligation": H.tax_obligation(obligation)})

    def prove_bracket(self, proof: H.ByteInput, inputs: Mapping[str, Any]):
        return self._call("prove_bracket", {"proof": H.hexbytes(proof), "inputs": H.age_proof_public_inputs(inputs)})

    def settle(self, subject: Mapping[str, Any], slot: H.Amount):
        return self._call("settle", {"subject": H.did(subject), "slot": H.amount(slot)})

    def authorize_audit(self, invoice: H.Bytes32, viewing_key_commitment: H.Bytes32):
        return self._call("authorize_audit", {"invoice": H.h32(invoice), "viewing_key_commitment": H.h32(viewing_key_commitment)})

    def set_brackets(self, brackets: Sequence[Mapping[str, Any]]):
        return self._call("set_brackets", {"brackets": [H.tax_bracket(b) for b in brackets]})


class TreasuryNs(_Ns):
    def mint(self, pool: int, amount: H.Amount):
        return self._call("mint", {"pool": pool, "amount": H.amount(amount)})

    def burn(self, amount: H.Amount):
        return self._call("burn", {"amount": H.amount(amount)})

    def subsidize(self, who: str, amount: H.Amount):
        return self._call("subsidize", {"who": who, "amount": H.amount(amount)})

    def record_settlement(self, receipt: H.Bytes32, amount: Mapping[str, Any]):
        return self._call("record_settlement", {"receipt": H.h32(receipt), "amount": H.fiat_amount(amount)})


class FederationNs(_Ns):
    def propose(self, action: Mapping[str, Any]):
        return self._call("propose", {"action": H.federation_action(action)})

    def vote(self, id: H.Amount, vote: str):
        return self._call("vote", {"id": H.amount(id), "vote": vote})

    def close(self, id: H.Amount):
        return self._call("close", {"id": H.amount(id)})

    def set_membership(self, member: str, seated: bool):
        return self._call("set_membership", {"member": H.fixed_ascii(member, 2), "seated": seated})

    def set_basket(self, basket: Mapping[str, Any]):
        return self._call("set_basket", {"basket": H.xsu_basket(basket)})

    def mint_xsu(self, cbdc: str, cbdc_amount: H.Amount):
        return self._call("mint_xsu", {"cbdc": H.fixed_ascii(cbdc, 3), "cbdc_amount": H.amount(cbdc_amount)})

    def redeem_xsu(self, cbdc: str, xsu_amount: H.Amount):
        return self._call("redeem_xsu", {"cbdc": H.fixed_ascii(cbdc, 3), "xsu_amount": H.amount(xsu_amount)})

    def book_clearing(self, to: str, amount: H.Amount):
        return self._call("book_clearing", {"to": H.fixed_ascii(to, 2), "amount": H.amount(amount)})

    def net_and_settle(self, window: int):
        return self._call("net_and_settle", {"window": window})

    def publish_proof_of_reserve(self):
        return self._call("publish_proof_of_reserve", {})


class InteropNs(_Ns):
    def register_issuer(self, entry: Mapping[str, Any]):
        return self._call("register_issuer", {"entry": H.trust_registry_entry(entry)})

    def submit_instruction(self, instr: Mapping[str, Any]):
        return self._call("submit_instruction", {"instr": H.clearing_instruction(instr)})

    def verify_finality(self, id: H.Amount, finality_proof: H.ByteInput):
        return self._call("verify_finality", {"id": H.amount(id), "finality_proof": H.hexbytes(finality_proof)})

    def net_and_settle(self, window: int):
        return self._call("net_and_settle", {"window": window})

    def register_validator(self, bond: H.Amount):
        return self._call("register_validator", {"bond": H.amount(bond)})

    def slash_validator(self, who: str, amount: H.Amount):
        return self._call("slash_validator", {"who": who, "amount": H.amount(amount)})

    def init_authority_set(self, country: str, authority_set: Mapping[str, Any]):
        return self._call("init_authority_set", {"country": H.fixed_ascii(country, 2), "set": H.grandpa_authority_set(authority_set)})

    def rotate_authority_set(self, country: str, finality_proof: H.ByteInput):
        return self._call("rotate_authority_set", {"country": H.fixed_ascii(country, 2), "finality_proof": H.hexbytes(finality_proof)})

    def register_issuer_vk(self, country: str, issuer_key_hash: H.Bytes32, vk: H.ByteInput):
        return self._call("register_issuer_vk", {"country": H.fixed_ascii(country, 2), "issuer_key_hash": H.h32(issuer_key_hash), "vk": H.hexbytes(vk)})

    def verify_foreign_proof(self, country: str, issuer_key_hash: H.Bytes32, proof: H.ByteInput, inputs: Mapping[str, Any]):
        return self._call("verify_foreign_proof", {
            "country": H.fixed_ascii(country, 2),
            "issuer_key_hash": H.h32(issuer_key_hash),
            "proof": H.hexbytes(proof),
            "inputs": H.age_proof_public_inputs(inputs),
        })

    def register_treaty(self, a: str, b: str, treaty: Mapping[str, Any]):
        return self._call("register_treaty", {"a": H.fixed_ascii(a, 2), "b": H.fixed_ascii(b, 2), "treaty": H.tax_treaty(treaty)})

    def recognize_foreign_invoice(self, country: str, invoice_hash: H.Bytes32):
        return self._call("recognize_foreign_invoice", {"country": H.fixed_ascii(country, 2), "invoice_hash": H.h32(invoice_hash)})

    def oss_register(self, subject: Mapping[str, Any], registration: Mapping[str, Any]):
        return self._call("oss_register", {"subject": H.did(subject), "registration": H.oss_registration(registration)})

    def oss_report(self, subject: Mapping[str, Any], to: str, amount: H.Amount, detail_commitment: H.Bytes32):
        return self._call("oss_report", {
            "subject": H.did(subject),
            "to": H.fixed_ascii(to, 2),
            "amount": H.amount(amount),
            "detail_commitment": H.h32(detail_commitment),
        })


class FerrumClient:
    """Thin typed wrapper over a connected SubstrateInterface."""

    def __init__(self, substrate: SubstrateInterface):
        self.substrate = substrate
        self.identity = IdentityNs(substrate, "Identity")
        self.credential = CredentialNs(substrate, "Credential")
        self.tax = TaxNs(substrate, "Tax")
        self.treasury = TreasuryNs(substrate, "Treasury")
        self.federation = FederationNs(substrate, "Federation")
        self.interop = InteropNs(substrate, "Interop")

    @classmethod
    def connect(cls, endpoint: str = DEFAULT_ENDPOINT) -> "FerrumClient":
        return cls(SubstrateInterface(url=endpoint))

    @staticmethod
    def keypair(uri: str = "//Alice", ss58_format: int = 42) -> Keypair:
        """Dev account (//Alice…) or `Keypair.create_from_mnemonic(...)`."""
        return Keypair.create_from_uri(uri, ss58_format=ss58_format)

    def sign_and_send(self, call, keypair: Keypair, wait_for_inclusion: bool = True):
        extrinsic = self.substrate.create_signed_extrinsic(call=call, keypair=keypair)
        receipt = self.substrate.submit_extrinsic(extrinsic, wait_for_inclusion=wait_for_inclusion)
        if wait_for_inclusion and not receipt.is_success:
            raise RuntimeError(f"extrinsic failed: {receipt.error_message}")
        return receipt

    def query(self, module: str, storage: str, params: Optional[Sequence[Any]] = None):
        return self.substrate.query(module=module, storage_function=storage, params=list(params or []))

    def subscribe_events(self, handler):
        """Block until interrupted, invoking handler(module, event, attributes)."""
        def _cb(obj, update_nr, subscription_id):
            for record in obj.value:
                ev = record["event"]
                handler(ev["module_id"], ev["event_id"], ev["attributes"])
        return self.substrate.subscribe_storage([("System", "Events")], _cb)

    def close(self):
        self.substrate.close()
