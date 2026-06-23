# Ferrum.Sdk — C# / .NET

Typed thin wrapper over [`Substrate.NetApi`](https://github.com/SubstrateGaming/Substrate.NET.API)
for the Ferrum sovereign blockchain. The SDK SCALE-encodes Ferrum call parameters
itself (verified against the runtime's pallet/call indices) and hands the resulting
`Method` to Substrate.NetApi, which assembles, signs and submits the extrinsic.

## Install

```bash
dotnet add package Ferrum.Sdk
# or reference sdk/csharp/Ferrum.Sdk/Ferrum.Sdk.csproj directly
```

## Quickstart

```csharp
using System.Numerics;
using Ferrum.Sdk;

await using var ferrum = await FerrumClient.ConnectAsync("ws://127.0.0.1:9944");
var issuer = FerrumClient.Account(secretKey, publicKey); // 32-byte sr25519 keys

var subject = Did.Of("tw", "citizen-0001");
var anchor = ferrum.Identity.AnchorDid(new DidDocument(
    Did: subject,
    Controller: publicKey,
    DocHashHex: "0x…",            // a commitment computed off-chain — no PII
    Keys: new[] { new DidKeyRef(KeyKind.Sr25519, "0x…") },
    RevocationCommitmentHex: "0x…",
    AnchoredAt: 0));

string hash = await ferrum.SignAndSendAsync(anchor, issuer);
```

Run the example against `ferrum-node --dev`:

```bash
dotnet run --project examples/Quickstart
```

## API shape

`FerrumClient` exposes one `*Calls` namespace per pallet; each method returns a
`FerrumCall` (pallet index + call index + SCALE params) submitted with
`SignAndSendAsync`:

```
ferrum.Identity     AnchorDid · RotateKeys · UpdateRevocation · RegisterIssuer
ferrum.Credential   Issue · Revoke · SetStatus · LogPresentation
ferrum.Tax          AnchorInvoice · Withhold · FileObligation · ProveBracket · Settle · AuthorizeAudit · SetBrackets
ferrum.Treasury     Mint · Burn · Subsidize · RecordSettlement
ferrum.Federation   Propose · Vote · Close · SetMembership · SetBasket · MintXsu · RedeemXsu · BookClearing · NetAndSettle · PublishProofOfReserve
ferrum.Interop      RegisterIssuer · SubmitInstruction · VerifyFinality · NetAndSettle · RegisterValidator · SlashValidator
                    InitAuthoritySet · RotateAuthoritySet · RegisterIssuerVk · VerifyForeignProof · RegisterTreaty
                    RecognizeForeignInvoice · OssRegister · OssReport
```

### Conventions

- 32-byte fields are `…Hex` strings (`0x…`); arbitrary blobs are `byte[]`.
- Tags/country/currency are short ASCII strings (`"tw"`, `"TW"`, `"TWD"`),
  length-checked at encode time.
- Variant fields use the C# enums (`KeyKind`, `TaxKind`, `Vote`, `CreditMethod`, …)
  whose values match the on-chain SCALE discriminants.
- Rates are `double` fractions of one (`0.05` = 5%); amounts are `BigInteger`.

Personal-data fields only accept commitments/hashes — by design you cannot put
plaintext PII into an extrinsic (whitepaper §03/§05/§06/§09).

> The example hashes with SHA-256 as a placeholder; use a BLAKE2b-256 library
> (e.g. `Blake2Fast`) for real commitments, matching the chain's hashing.
