"""Converters from friendly Python inputs to the dicts substrate-interface wants
for the Ferrum SCALE types.

The Ferrum runtime ships full SCALE-info metadata (V15), so substrate-interface
encodes/decodes every type automatically once connected; these helpers just shape
the call params so callers don't have to memorize field layouts.
"""
from __future__ import annotations

from typing import Any, Mapping, Optional, Sequence, Union

Bytes32 = Union[str, bytes]
ByteInput = Union[str, bytes]
Amount = Union[int, str]


def _to_bytes(v: ByteInput) -> bytes:
    if isinstance(v, bytes):
        return v
    s = v[2:] if v.startswith("0x") else v
    return bytes.fromhex(s)


def h32(v: Bytes32) -> str:
    """Normalize a 32-byte value to a 0x-hex string; raise on wrong length."""
    b = _to_bytes(v)
    if len(b) != 32:
        raise ValueError(f"expected 32 bytes, got {len(b)}")
    return "0x" + b.hex()


def hexbytes(v: ByteInput) -> str:
    """Normalize an arbitrary byte blob (proof/vk/finality) to 0x-hex."""
    return "0x" + _to_bytes(v).hex()


def ascii_hex(s: str) -> str:
    """Encode a short ASCII string (tag) to 0x-hex bytes for a BoundedVec<u8>."""
    return "0x" + s.encode("ascii").hex()


def fixed_ascii(s: str, length: int) -> list[int]:
    """Encode a fixed-width ASCII code (e.g. 'TWD'->3) as a byte array."""
    b = s.encode("ascii")
    if len(b) != length:
        raise ValueError(f'code "{s}" must be exactly {length} ASCII bytes')
    return list(b)


def perbill(frac: float) -> int:
    """Fraction of one (0..1) -> Perbill parts-per-billion integer."""
    if not 0 <= frac <= 1:
        raise ValueError("perbill fraction must be within [0, 1]")
    return round(frac * 1_000_000_000)


def amount(a: Amount) -> int:
    return int(a)


def did(d: Mapping[str, Any]) -> dict:
    raw = d["id"]
    if isinstance(raw, str) and not raw.startswith("0x"):
        ident = ascii_hex(raw)
    else:
        ident = hexbytes(raw)
    return {"chain_tag": ascii_hex(d["chain_tag"]), "id": ident}


def did_key_ref(k: Mapping[str, Any]) -> dict:
    return {"kind": k["kind"], "key_hash": h32(k["key_hash"])}


def did_document(d: Mapping[str, Any]) -> dict:
    return {
        "did": did(d["did"]),
        "controller": d["controller"],
        "doc_hash": h32(d["doc_hash"]),
        "keys": [did_key_ref(k) for k in d["keys"]],
        "revocation_commitment": h32(d["revocation_commitment"]),
        "anchored_at": d["anchored_at"],
    }


def fiat_amount(f: Mapping[str, Any]) -> dict:
    return {"currency": fixed_ascii(f["currency"], 3), "minor_units": amount(f["minor_units"])}


def credential_anchor(c: Mapping[str, Any]) -> dict:
    expires = c.get("expires_at")
    return {
        "subject": did(c["subject"]),
        "issuer": c["issuer"],
        "kind": c["kind"],
        "payload_hash": h32(c["payload_hash"]),
        "status": c["status"],
        "expires_at": None if expires is None else amount(expires),
    }


def tax_bracket(b: Mapping[str, Any]) -> dict:
    return {"index": b["index"], "rate": perbill(b["rate"])}


def invoice_anchor(i: Mapping[str, Any]) -> dict:
    return {
        "invoice_hash": h32(i["invoice_hash"]),
        "issuer": i["issuer"],
        "kind": i["kind"],
        "anchored_at": amount(i["anchored_at"]),
    }


def tax_obligation(o: Mapping[str, Any]) -> dict:
    return {
        "subject": did(o["subject"]),
        "kind": o["kind"],
        "amount_due": fiat_amount(o["amount_due"]),
        "detail_commitment": h32(o["detail_commitment"]),
        "settled": o["settled"],
    }


def age_proof_public_inputs(p: Mapping[str, Any]) -> dict:
    return {
        "issuer_commitment": h32(p["issuer_commitment"]),
        "threshold": p["threshold"],
        "nullifier": h32(p["nullifier"]),
    }


def xsu_basket(b: Mapping[str, Any]) -> dict:
    return {
        "entries": [{"cbdc": fixed_ascii(e["cbdc"], 3), "weight": perbill(e["weight"])} for e in b["entries"]],
        "version": b["version"],
    }


def federation_action(a: Mapping[str, Any]) -> dict:
    t = a["type"]
    if t == "SetParameter":
        return {"SetParameter": {"key": ascii_hex(a["key"]), "value": amount(a["value"])}}
    if t in ("AdmitMember", "RemoveMember", "SuspendMember"):
        return {t: {"member": fixed_ascii(a["member"], 2)}}
    if t == "Reweight":
        return {"Reweight": {"basket": xsu_basket(a["basket"])}}
    if t == "RuntimeUpgrade":
        return {"RuntimeUpgrade": {"code_hash": h32(a["code_hash"])}}
    raise ValueError(f"unknown federation action: {t}")


def trust_registry_entry(e: Mapping[str, Any]) -> dict:
    return {
        "country": fixed_ascii(e["country"], 2),
        "issuer_key_hash": h32(e["issuer_key_hash"]),
        "scope": ascii_hex(e["scope"]),
        "active": e["active"],
    }


def clearing_instruction(c: Mapping[str, Any]) -> dict:
    return {
        "from": fixed_ascii(c["from"], 2),
        "to": fixed_ascii(c["to"], 2),
        "amount": amount(c["amount"]),
        "detail_commitment": h32(c["detail_commitment"]),
        "status": c.get("status", "Pending"),
    }


def tax_treaty(t: Mapping[str, Any]) -> dict:
    return {"withholding_cap": perbill(t["withholding_cap"]), "method": t["method"], "active": t["active"]}


def oss_registration(r: Mapping[str, Any]) -> dict:
    return {"home": fixed_ascii(r["home"], 2), "vat_id_commitment": h32(r["vat_id_commitment"]), "active": r["active"]}


def grandpa_authority_set(s: Mapping[str, Any]) -> dict:
    return {
        "authorities": [{"id": h32(a["id"]), "weight": amount(a["weight"])} for a in s["authorities"]],
        "set_id": amount(s["set_id"]),
    }


# Treasury allocation pools (§08).
POOLS = {"staking": 0, "treasury": 1, "subsidy": 2, "dev": 3, "ecosystem": 4}
