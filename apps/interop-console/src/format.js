// Formatting + small deterministic helpers for the interop console.
// Mirrors ferrum-primitives units: FER = 10^12, XSU minor units = 10^6.

export const FER_DECIMALS = 12
export const XSU_DECIMALS = 6
export const FER = 10n ** BigInt(FER_DECIMALS)
export const MIN_VALIDATOR_BOND = 250_000n * FER // §07 / §11.1

// Country codes are 2-byte ASCII (CountryId). The local chain tag is lowercase.
export const LOCAL_TAG = 'tw'

export const COUNTRIES = {
  TW: { flag: '🇹🇼', zh: '臺灣', en: 'Taiwan', cbdc: 'eTWD' },
  JP: { flag: '🇯🇵', zh: '日本', en: 'Japan', cbdc: 'eJPY' },
  DE: { flag: '🇩🇪', zh: '德國', en: 'Germany', cbdc: 'eEUR' },
  US: { flag: '🇺🇸', zh: '美國', en: 'United States', cbdc: 'eUSD' },
  CN: { flag: '🇨🇳', zh: '中國', en: 'China', cbdc: 'eCNY' },
}

// The chain this console operates (CountryId form of LOCAL_TAG).
export const LOCAL_COUNTRY = LOCAL_TAG.toUpperCase()
// All federation members and the foreign subset — derived from COUNTRIES so a new
// country only needs to be added in one place (the COUNTRIES map above).
export const ALL_COUNTRIES = Object.keys(COUNTRIES)
export const FOREIGN_COUNTRIES = ALL_COUNTRIES.filter((c) => c !== LOCAL_COUNTRY)

export function countryLabel(code, lang = 'en') {
  const c = COUNTRIES[code]
  if (!c) return code
  return `${c.flag} ${code}`
}

export function countryName(code, lang = 'en') {
  const c = COUNTRIES[code]
  return c ? (lang === 'zh' ? c.zh : c.en) : code
}

// Format an integer-minor XSU amount (BigInt or number) as a decimal string.
export function fmtXsu(minor) {
  return fmtMinor(BigInt(minor), XSU_DECIMALS)
}

// Format an integer-planck FER amount as a human FER string.
export function fmtFer(planck) {
  return fmtMinor(BigInt(planck), FER_DECIMALS)
}

function fmtMinor(value, decimals) {
  const neg = value < 0n
  let v = neg ? -value : value
  const base = 10n ** BigInt(decimals)
  const whole = v / base
  const frac = v % base
  let s = groupThousands(whole.toString())
  if (frac > 0n) {
    let f = frac.toString().padStart(decimals, '0').replace(/0+$/, '')
    s += '.' + f
  }
  return (neg ? '-' : '') + s
}

function groupThousands(s) {
  return s.replace(/\B(?=(\d{3})+(?!\d))/g, ',')
}

// Shorten a 0x hash for display.
export function shortHash(h, head = 10, tail = 6) {
  if (!h) return '—'
  if (h.length <= head + tail + 2) return h
  return `${h.slice(0, head)}…${h.slice(-tail)}`
}

// Deterministic pseudo-hash from a seed string (FNV-1a expanded to 32 bytes).
// Purely for display fixtures — never used as real cryptographic material.
export function pseudoHash(seed, bytes = 32) {
  let h = 0x811c9dc5 >>> 0
  const out = []
  for (let i = 0; i < bytes; i++) {
    for (let j = 0; j < seed.length; j++) {
      h ^= seed.charCodeAt(j) + i * 131
      h = Math.imul(h, 0x01000193) >>> 0
    }
    out.push((h & 0xff).toString(16).padStart(2, '0'))
  }
  return '0x' + out.join('')
}

export function randHash(bytes = 32) {
  const a = new Uint8Array(bytes)
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) crypto.getRandomValues(a)
  return '0x' + Array.from(a, (b) => b.toString(16).padStart(2, '0')).join('')
}

// Parse a user-entered decimal amount into integer minor units (BigInt).
export function parseMinor(input, decimals) {
  const s = String(input).trim()
  if (!s) return 0n
  const [whole, frac = ''] = s.split('.')
  const fracPadded = (frac + '0'.repeat(decimals)).slice(0, decimals)
  const sign = whole.startsWith('-') ? -1n : 1n
  const wAbs = whole.replace('-', '') || '0'
  return sign * (BigInt(wAbs) * 10n ** BigInt(decimals) + BigInt(fracPadded || '0'))
}

export const parseXsu = (s) => parseMinor(s, XSU_DECIMALS)
export const parseFer = (s) => parseMinor(s, FER_DECIMALS)
