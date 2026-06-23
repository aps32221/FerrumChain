# ferrum-sdk — Python

Typed thin wrapper over [`substrate-interface`](https://github.com/polkascan/py-substrate-interface)
for the Ferrum sovereign blockchain.

## Install

```bash
pip install ferrum-sdk          # or:  pip install -e .   (from this directory)
```

## Quickstart

```python
import hashlib
from ferrum_sdk import FerrumClient

ferrum = FerrumClient.connect("ws://127.0.0.1:9944")
alice = ferrum.keypair("//Alice")

def commit(s): return "0x" + hashlib.blake2b(s.encode(), digest_size=32).hexdigest()

call = ferrum.identity.anchor_did({
    "did": {"chain_tag": "tw", "id": "citizen-0001"},
    "controller": alice.ss58_address,
    "doc_hash": commit("off-chain DID document"),     # commitment only — no PII
    "keys": [{"kind": "Sr25519", "key_hash": commit("device-key")}],
    "revocation_commitment": commit("rev-acc-0"),
    "anchored_at": 0,
})
ferrum.sign_and_send(call, alice)
ferrum.close()
```

Run the full example against `ferrum-node --dev`:

```bash
pip install -e .
python examples/quickstart.py
```

## API shape

`FerrumClient` exposes one namespace per pallet; each method returns a `GenericCall`
you submit with `sign_and_send`:

```
ferrum.identity     anchor_did · rotate_keys · update_revocation · register_issuer
ferrum.credential   issue · revoke · set_status · log_presentation
ferrum.tax          anchor_invoice · withhold · file_obligation · prove_bracket · settle · authorize_audit · set_brackets
ferrum.treasury     mint · burn · subsidize · record_settlement
ferrum.federation   propose · vote · close · set_membership · set_basket · mint_xsu · redeem_xsu · book_clearing · net_and_settle · publish_proof_of_reserve
ferrum.interop      register_issuer · submit_instruction · verify_finality · net_and_settle · register_validator · slash_validator
                    init_authority_set · rotate_authority_set · register_issuer_vk · verify_foreign_proof · register_treaty
                    recognize_foreign_invoice · oss_register · oss_report
```

### Storage and events

```python
doc = ferrum.query("Identity", "Dids", [{"chain_tag": "tw", "id": "citizen-0001"}])
burned = ferrum.query("Treasury", "TotalBurned")

ferrum.subscribe_events(lambda module, event, attrs: print(module, event, attrs))
```

### Input conventions

- 32-byte fields accept a `0x…` hex string or `bytes`.
- Tags/country/currency accept short ASCII strings (`"tw"`, `"TW"`, `"TWD"`).
- Rates (`Perbill`) accept a fraction of one: `0.05` = 5%.
- Amounts accept `int` or a decimal string (FER has 12 decimals).

Personal-data fields only accept commitments/hashes — you cannot put plaintext PII
into an extrinsic (whitepaper §03/§05/§06/§09).
