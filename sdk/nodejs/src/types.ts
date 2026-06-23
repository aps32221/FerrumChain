/**
 * Ferrum value-type definitions and friendly input shapes.
 *
 * The Ferrum runtime ships full SCALE-info metadata (V15), so `@polkadot/api`
 * encodes/decodes every type below automatically once connected. These TypeScript
 * interfaces describe the *friendly* inputs the SDK helpers accept; the helpers in
 * `helpers.ts` convert them to the registry-friendly objects `@polkadot/api` wants.
 */

/** A 32-byte value (hash/commitment/nullifier): `0x…` hex string or raw bytes. */
export type Bytes32 = string | Uint8Array;
/** Arbitrary byte blob (proof, vk, finality proof): `0x…` hex or raw bytes. */
export type ByteInput = string | Uint8Array;
/** SS58-encoded account address. */
export type Address = string;
/** A balance / amount — pass bigint, number or decimal string. */
export type Amount = bigint | number | string;

export type KeyKind = "Sr25519" | "Ed25519" | "Bls12_381";
export type CredentialKind = "Nationality" | "Age" | "Residence" | "TaxStatus" | "Other";
export type CredentialStatus = "Active" | "Suspended" | "Revoked" | "Expired";
export type TaxKind = "Income" | "Wage" | "Interest" | "ValueAdded" | "Withholding" | "Other";
export type Vote = "Aye" | "Nay" | "Abstain";
export type XcmStatus = "Pending" | "FinalityVerified" | "Accepted" | "Rejected";
export type CreditMethod = "Credit" | "Exemption";

/** `did:fer:<chainTag>:<id>` — pass tag/id as ASCII strings; bytes are derived. */
export interface DidInput {
  /** Country/source-chain tag, e.g. "tw", "jp". */
  chainTag: string;
  /** Method-specific identifier (ASCII string or `0x…` bytes). */
  id: string | Uint8Array;
}

export interface DidKeyRefInput {
  kind: KeyKind;
  keyHash: Bytes32;
}

export interface DidDocumentInput {
  did: DidInput;
  controller: Address;
  docHash: Bytes32;
  keys: DidKeyRefInput[];
  revocationCommitment: Bytes32;
  anchoredAt: number;
}

export interface CredentialAnchorInput {
  subject: DidInput;
  issuer: Address;
  kind: CredentialKind;
  payloadHash: Bytes32;
  status: CredentialStatus;
  /** Milliseconds since epoch, or null for no expiry. */
  expiresAt?: number | bigint | null;
}

/** A fiat amount: 3-letter currency (e.g. "TWD") + minor units (e.g. cents). */
export interface FiatAmountInput {
  currency: string;
  minorUnits: Amount;
}

export interface TaxBracketInput {
  index: number;
  /** Rate as a fraction of one (0..1), e.g. 0.05 for 5%. */
  rate: number;
}

export interface InvoiceAnchorInput {
  invoiceHash: Bytes32;
  issuer: Address;
  kind: TaxKind;
  /** Milliseconds since epoch. */
  anchoredAt: number | bigint;
}

export interface TaxObligationInput {
  subject: DidInput;
  kind: TaxKind;
  amountDue: FiatAmountInput;
  detailCommitment: Bytes32;
  settled: boolean;
}

export interface AgeProofPublicInputs {
  issuerCommitment: Bytes32;
  threshold: number;
  nullifier: Bytes32;
}

export interface BasketEntryInput {
  /** CBDC code, e.g. "USD". */
  cbdc: string;
  /** Weight as a fraction of one (0..1). */
  weight: number;
}

export interface XsuBasketInput {
  entries: BasketEntryInput[];
  version: number;
}

/** A federation action subject to dual-majority governance (§11.4). */
export type FederationActionInput =
  | { type: "SetParameter"; key: string; value: Amount }
  | { type: "AdmitMember"; member: string }
  | { type: "RemoveMember"; member: string }
  | { type: "Reweight"; basket: XsuBasketInput }
  | { type: "SuspendMember"; member: string }
  | { type: "RuntimeUpgrade"; codeHash: Bytes32 };

export interface TrustRegistryEntryInput {
  /** Country code, e.g. "TW". */
  country: string;
  issuerKeyHash: Bytes32;
  /** Treaty scope tag (ASCII, never PII). */
  scope: string;
  active: boolean;
}

export interface ClearingInstructionInput {
  from: string;
  to: string;
  amount: Amount;
  detailCommitment: Bytes32;
  status?: XcmStatus;
}

export interface TaxTreatyInput {
  /** Withholding cap as a fraction of one (0..1). */
  withholdingCap: number;
  method: CreditMethod;
  active: boolean;
}

export interface OssRegistrationInput {
  home: string;
  vatIdCommitment: Bytes32;
  active: boolean;
}

export interface GrandpaAuthorityInput {
  id: Bytes32;
  weight: number | bigint;
}

export interface GrandpaAuthoritySetInput {
  authorities: GrandpaAuthorityInput[];
  setId: number | bigint;
}

/** Treasury allocation pools (§08). */
export const Pools = {
  staking: 0,
  treasury: 1,
  subsidy: 2,
  dev: 3,
  ecosystem: 4,
} as const;
