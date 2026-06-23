/**
 * FerrumClient — a typed thin wrapper over `@polkadot/api`'s `ApiPromise`.
 *
 * Every method returns a `SubmittableExtrinsic` so callers keep full control of
 * signing, mortality, tips and nonce. Use `signAndSend` for the common path.
 */
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady } from "@polkadot/util-crypto";
import type { SubmittableExtrinsic } from "@polkadot/api/types";
import type { ISubmittableResult } from "@polkadot/types/types";
import type { KeyringPair } from "@polkadot/keyring/types";
import * as H from "./helpers.js";
import type {
  DidInput, DidKeyRefInput, DidDocumentInput, CredentialAnchorInput, CredentialStatus,
  FiatAmountInput, TaxBracketInput, InvoiceAnchorInput, TaxObligationInput, AgeProofPublicInputs,
  XsuBasketInput, FederationActionInput, Vote, TrustRegistryEntryInput, ClearingInstructionInput,
  TaxTreatyInput, OssRegistrationInput, GrandpaAuthoritySetInput, Bytes32, ByteInput, Address,
  Amount, TaxKind,
} from "./types.js";

export const DEFAULT_ENDPOINT = "ws://127.0.0.1:9944";
export type Tx = SubmittableExtrinsic<"promise", ISubmittableResult>;

export interface ConnectOptions {
  endpoint?: string;
  /** Pass an existing ApiPromise instead of dialing a new connection. */
  api?: ApiPromise;
}

export class FerrumClient {
  readonly identity: IdentityNs;
  readonly credential: CredentialNs;
  readonly tax: TaxNs;
  readonly treasury: TreasuryNs;
  readonly federation: FederationNs;
  readonly interop: InteropNs;

  private constructor(public readonly api: ApiPromise) {
    this.identity = new IdentityNs(api);
    this.credential = new CredentialNs(api);
    this.tax = new TaxNs(api);
    this.treasury = new TreasuryNs(api);
    this.federation = new FederationNs(api);
    this.interop = new InteropNs(api);
  }

  static async connect(opts: ConnectOptions = {}): Promise<FerrumClient> {
    await cryptoWaitReady();
    const api = opts.api ?? (await ApiPromise.create({
      provider: new WsProvider(opts.endpoint ?? DEFAULT_ENDPOINT),
    }));
    await api.isReady;
    return new FerrumClient(api);
  }

  /** Build a sr25519 signer from a mnemonic, seed or dev URI (e.g. "//Alice"). */
  keypair(uri: string, ss58Format = 42): KeyringPair {
    return new Keyring({ type: "sr25519", ss58Format }).addFromUri(uri);
  }

  /** Sign and submit a tx; resolves on in-block inclusion with the events. */
  signAndSend(tx: Tx, signer: KeyringPair): Promise<ISubmittableResult> {
    return new Promise((resolve, reject) => {
      tx.signAndSend(signer, (result) => {
        const { status, dispatchError } = result;
        if (dispatchError) {
          if (dispatchError.isModule) {
            const d = this.api.registry.findMetaError(dispatchError.asModule);
            return reject(new Error(`${d.section}.${d.name}: ${d.docs.join(" ")}`));
          }
          return reject(new Error(dispatchError.toString()));
        }
        if (status.isInBlock || status.isFinalized) resolve(result);
      }).catch(reject);
    });
  }

  /** Read storage. Mirrors `api.query.<pallet>.<item>(...keys)`. */
  get query() {
    return this.api.query;
  }

  /** Subscribe to all chain events; returns an unsubscribe function. */
  async subscribeEvents(handler: (event: { section: string; method: string; data: unknown }) => void): Promise<() => void> {
    const unsub = await this.api.query.system.events((records: any) => {
      for (const { event } of records) {
        handler({ section: event.section, method: event.method, data: event.data.toHuman() });
      }
    });
    return unsub as unknown as () => void;
  }

  async disconnect(): Promise<void> {
    await this.api.disconnect();
  }
}

// ---------------------------------------------------------------------------
// Pallet namespaces — each method returns a SubmittableExtrinsic.
// ---------------------------------------------------------------------------

class IdentityNs {
  constructor(private api: ApiPromise) {}
  /** Anchor a `did:fer` DID document (issuer origin). */
  anchorDid(doc: DidDocumentInput): Tx { return this.api.tx.identity.anchorDid(H.didDocument(doc)); }
  /** Rotate the verification keys of a DID (controller origin). */
  rotateKeys(d: DidInput, keys: DidKeyRefInput[]): Tx { return this.api.tx.identity.rotateKeys(H.did(d), keys.map(H.didKeyRef)); }
  /** Update the revocation accumulator commitment (issuer origin). */
  updateRevocation(commitment: Bytes32): Tx { return this.api.tx.identity.updateRevocation(H.h32(commitment)); }
  /** Accredit an issuer account (governance origin). */
  registerIssuer(who: Address): Tx { return this.api.tx.identity.registerIssuer(who); }
}

class CredentialNs {
  constructor(private api: ApiPromise) {}
  /** Anchor a verifiable-credential hash (issuer origin). */
  issue(anchor: CredentialAnchorInput): Tx { return this.api.tx.credential.issue(H.credentialAnchor(anchor)); }
  /** Revoke a credential by payload hash (issuer origin). */
  revoke(payloadHash: Bytes32): Tx { return this.api.tx.credential.revoke(H.h32(payloadHash)); }
  /** Set a credential's lifecycle status (issuer origin). */
  setStatus(payloadHash: Bytes32, status: CredentialStatus): Tx { return this.api.tx.credential.setStatus(H.h32(payloadHash), status); }
  /** Log a one-time selective-disclosure presentation (replay-protected). */
  logPresentation(nullifier: Bytes32, commitment: Bytes32): Tx { return this.api.tx.credential.logPresentation(H.h32(nullifier), H.h32(commitment)); }
}

class TaxNs {
  constructor(private api: ApiPromise) {}
  /** Anchor an e-invoice hash. */
  anchorInvoice(anchor: InvoiceAnchorInput): Tx { return this.api.tx.tax.anchorInvoice(H.invoiceAnchor(anchor)); }
  /** Record programmable withholding against a subject DID. */
  withhold(subject: DidInput, kind: TaxKind, amount: FiatAmountInput): Tx { return this.api.tx.tax.withhold(H.did(subject), kind, H.fiatAmount(amount)); }
  /** File a (fiat-denominated) tax obligation. */
  fileObligation(obligation: TaxObligationInput): Tx { return this.api.tx.tax.fileObligation(H.taxObligation(obligation)); }
  /** Submit a ZK tax-bracket proof. */
  proveBracket(proof: ByteInput, inputs: AgeProofPublicInputs): Tx { return this.api.tx.tax.proveBracket(H.bytes(proof), H.ageProofPublicInputs(inputs)); }
  /** Settle an obligation slot in eTWD CBDC. */
  settle(subject: DidInput, slot: Amount): Tx { return this.api.tx.tax.settle(H.did(subject), H.amount(slot)); }
  /** Authorize a scoped audit of an invoice (auditor origin). */
  authorizeAudit(invoice: Bytes32, viewingKeyCommitment: Bytes32): Tx { return this.api.tx.tax.authorizeAudit(H.h32(invoice), H.h32(viewingKeyCommitment)); }
  /** Replace the tax-bracket table (governance origin). */
  setBrackets(brackets: TaxBracketInput[]): Tx { return this.api.tx.tax.setBrackets(brackets.map(H.taxBracket)); }
}

class TreasuryNs {
  constructor(private api: ApiPromise) {}
  /** Mint FER into an allocation pool (governance origin). */
  mint(pool: number, amount: Amount): Tx { return this.api.tx.treasury.mint(pool, H.amount(amount)); }
  /** Base-fee burn of FER. */
  burn(amount: Amount): Tx { return this.api.tx.treasury.burn(H.amount(amount)); }
  /** Subsidize an account from the subsidy pool (governance origin). */
  subsidize(who: Address, amount: Amount): Tx { return this.api.tx.treasury.subsidize(who, H.amount(amount)); }
  /** Record an eTWD tax settlement receipt (from pallet-tax). */
  recordSettlement(receipt: Bytes32, amount: FiatAmountInput): Tx { return this.api.tx.treasury.recordSettlement(H.h32(receipt), H.fiatAmount(amount)); }
}

class FederationNs {
  constructor(private api: ApiPromise) {}
  /** Propose a federation action (council member origin). */
  propose(action: FederationActionInput): Tx { return this.api.tx.federation.propose(H.federationAction(action)); }
  /** Cast a vote on a proposal (council member origin). */
  vote(id: Amount, vote: Vote): Tx { return this.api.tx.federation.vote(H.amount(id), vote); }
  /** Close a proposal: runs dual-majority and queues under timelock. */
  close(id: Amount): Tx { return this.api.tx.federation.close(H.amount(id)); }
  /** Seat or unseat a council member (council member origin). */
  setMembership(member: string, seated: boolean): Tx { return this.api.tx.federation.setMembership(H.fixedAscii(member, 2), seated); }
  /** Replace the active XSU basket (council member origin). */
  setBasket(basket: XsuBasketInput): Tx { return this.api.tx.federation.setBasket(H.xsuBasket(basket)); }
  /** Mint XSU against a deposited CBDC amount. */
  mintXsu(cbdc: string, cbdcAmount: Amount): Tx { return this.api.tx.federation.mintXsu(H.fixedAscii(cbdc, 3), H.amount(cbdcAmount)); }
  /** Redeem XSU back into a CBDC. */
  redeemXsu(cbdc: string, xsuAmount: Amount): Tx { return this.api.tx.federation.redeemXsu(H.fixedAscii(cbdc, 3), H.amount(xsuAmount)); }
  /** Book a cross-member clearing instruction priced in XSU. */
  bookClearing(to: string, amount: Amount): Tx { return this.api.tx.federation.bookClearing(H.fixedAscii(to, 2), H.amount(amount)); }
  /** Net and settle a clearing window. */
  netAndSettle(window: number): Tx { return this.api.tx.federation.netAndSettle(window); }
  /** Publish a proof-of-reserve digest. */
  publishProofOfReserve(): Tx { return this.api.tx.federation.publishProofOfReserve(); }
}

class InteropNs {
  constructor(private api: ApiPromise) {}
  /** Register a cross-chain trust-registry issuer (federation origin). */
  registerIssuer(entry: TrustRegistryEntryInput): Tx { return this.api.tx.interop.registerIssuer(H.trustRegistryEntry(entry)); }
  /** Submit an XSU-priced clearing instruction (relayer origin). */
  submitInstruction(instr: ClearingInstructionInput): Tx { return this.api.tx.interop.submitInstruction(H.clearingInstruction(instr)); }
  /** Verify a foreign GRANDPA finality proof for an instruction (relayer origin). */
  verifyFinality(id: Amount, finalityProof: ByteInput): Tx { return this.api.tx.interop.verifyFinality(H.amount(id), H.bytes(finalityProof)); }
  /** Net and settle interop positions for a window (federation origin). */
  netAndSettle(window: number): Tx { return this.api.tx.interop.netAndSettle(window); }
  /** Bond and register as an interop validator. */
  registerValidator(bond: Amount): Tx { return this.api.tx.interop.registerValidator(H.amount(bond)); }
  /** Slash an interop validator (federation origin). */
  slashValidator(who: Address, amount: Amount): Tx { return this.api.tx.interop.slashValidator(who, H.amount(amount)); }
  /** Initialize a bridged chain's GRANDPA authority set (federation origin). */
  initAuthoritySet(country: string, set: GrandpaAuthoritySetInput): Tx { return this.api.tx.interop.initAuthoritySet(H.fixedAscii(country, 2), H.grandpaAuthoritySet(set)); }
  /** Rotate a bridged chain's authority set via a finality proof (relayer origin). */
  rotateAuthoritySet(country: string, finalityProof: ByteInput): Tx { return this.api.tx.interop.rotateAuthoritySet(H.fixedAscii(country, 2), H.bytes(finalityProof)); }
  /** Register a recognized foreign issuer's ZK verifying key (federation origin). */
  registerIssuerVk(country: string, issuerKeyHash: Bytes32, vk: ByteInput): Tx { return this.api.tx.interop.registerIssuerVk(H.fixedAscii(country, 2), H.h32(issuerKeyHash), H.bytes(vk)); }
  /** Verify a foreign selective-disclosure proof (relayer origin). */
  verifyForeignProof(country: string, issuerKeyHash: Bytes32, proof: ByteInput, inputs: AgeProofPublicInputs): Tx {
    return this.api.tx.interop.verifyForeignProof(H.fixedAscii(country, 2), H.h32(issuerKeyHash), H.bytes(proof), H.ageProofPublicInputs(inputs));
  }
  /** Register a bilateral tax treaty (federation origin). */
  registerTreaty(a: string, b: string, treaty: TaxTreatyInput): Tx { return this.api.tx.interop.registerTreaty(H.fixedAscii(a, 2), H.fixedAscii(b, 2), H.taxTreaty(treaty)); }
  /** Recognize a foreign e-invoice after finality (relayer origin). */
  recognizeForeignInvoice(country: string, invoiceHash: Bytes32): Tx { return this.api.tx.interop.recognizeForeignInvoice(H.fixedAscii(country, 2), H.h32(invoiceHash)); }
  /** Register a One-Stop-Shop VAT registration. */
  ossRegister(subject: DidInput, registration: OssRegistrationInput): Tx { return this.api.tx.interop.ossRegister(H.did(subject), H.ossRegistration(registration)); }
  /** File an OSS VAT report allocating revenue to a destination country. */
  ossReport(subject: DidInput, to: string, amount: Amount, detailCommitment: Bytes32): Tx {
    return this.api.tx.interop.ossReport(H.did(subject), H.fixedAscii(to, 2), H.amount(amount), H.h32(detailCommitment));
  }
}
