# Ferrum 身分證核發終端 (ID Issuer)

A clean WPF desktop terminal for issuing a Ferrum digital identity. Fill in the
holder's details, press **核發**, and the app shows the formatted ID card and a
verification **QR code**. Optionally anchor the DID on-chain.

```
dotnet run --project apps/id-issuer
```

Requires the .NET 8 SDK with the Windows desktop workload (`Microsoft.NET.Sdk` +
`UseWPF`). Windows-only.

## What it does

1. **Issue (offline).** Builds `did:fer:<tag>:<id>` and a 32-byte document
   commitment, renders the card, and encodes a compact JSON payload as a QR code
   that a verifier can scan and re-hash against the chain.
2. **Anchor (optional).** Under *進階：DID 上鏈*, supply the node endpoint and the
   issuer's sr25519 keys (32-byte secret/public hex) to submit
   `Identity.anchor_did` via the Ferrum C# SDK. The issuer account must already be
   accredited (`registerIssuer`).

## Privacy

The chain stores only the commitment hash — never the personal fields. The QR
payload carries the human-readable card for offline display; in a production
deployment sign that payload and apply your jurisdiction's PII policy.

> The commitment uses SHA-256 here to match the SDK quickstart placeholder; the
> runtime expects BLAKE2b-256, so swap in a BLAKE2b implementation before real use.

## Layout

| File | Purpose |
|------|---------|
| `MainWindow.xaml(.cs)` | UI + issue / clear / anchor handlers |
| `Services/IdCard.cs` | DID + commitment + QR payload |
| `Services/QrCodeService.cs` | QRCoder → WPF `BitmapImage` |
| `Services/ChainService.cs` | optional `anchor_did` via `Ferrum.Sdk` |
