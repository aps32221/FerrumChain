// Formatting + helpers for the federation governance console.
// XSU minor units = 10^6; FER = 10^12; basket weights are Perbill (ppb, 10^9).

export const XSU_DECIMALS = 6
export const FER_DECIMALS = 12
export const PPB = 1_000_000_000 // Perbill denominator (parts-per-billion)
export const FER = 10n ** BigInt(FER_DECIMALS)
export const MIN_VALIDATOR_BOND = 250_000n * FER

// The council seat this console operates.
export const LOCAL_MEMBER = 'TW'

// Federation members: 2-byte MemberId → flag, names, and 3-byte CBDC code.
export const MEMBERS = {
  TW: { flag: '🇹🇼', zh: '臺灣', en: 'Taiwan', cbdc: 'TWD', etoken: 'eTWD' },
  JP: { flag: '🇯🇵', zh: '日本', en: 'Japan', cbdc: 'JPY', etoken: 'eJPY' },
  US: { flag: '🇺🇸', zh: '美國', en: 'United States', cbdc: 'USD', etoken: 'eUSD' },
  DE: { flag: '🇩🇪', zh: '德國', en: 'Germany', cbdc: 'EUR', etoken: 'eEUR' },
  CN: { flag: '🇨🇳', zh: '中國', en: 'China', cbdc: 'CNY', etoken: 'eCNY' },
  KR: { flag: '🇰🇷', zh: '韓國', en: 'South Korea', cbdc: 'KRW', etoken: 'eKRW' },
}

export const ALL_MEMBERS = Object.keys(MEMBERS)
export const FOREIGN_MEMBERS = ALL_MEMBERS.filter((m) => m !== LOCAL_MEMBER)
// CBDC codes present in members (for basket/reserve selectors).
export const CBDC_CODES = ALL_MEMBERS.map((m) => MEMBERS[m].cbdc)

export function memberLabel(code) {
  const m = MEMBERS[code]
  return m ? `${m.flag} ${code}` : code
}
export function memberName(code, lang = 'en') {
  const m = MEMBERS[code]
  return m ? (lang === 'zh' ? m.zh : m.en) : code
}
// Map a 3-byte CBDC code back to its MemberId (e.g. USD → US).
export function cbdcToMember(cbdc) {
  return ALL_MEMBERS.find((m) => MEMBERS[m].cbdc === cbdc) || cbdc
}

// Perbill (ppb) → percentage string.
export function fmtPpb(ppb) {
  return (Number(ppb) / PPB) * 100
}
export function fmtPct(ppb, digits = 1) {
  return `${fmtPpb(ppb).toFixed(digits)}%`
}
export function pctToPpb(pct) {
  return Math.round((Number(pct) / 100) * PPB)
}

// Amount formatting (XSU / CBDC reserve / FER all integer-minor BigInts).
export function fmtXsu(minor) {
  return fmtMinor(BigInt(minor), XSU_DECIMALS)
}
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
  if (frac > 0n) s += '.' + frac.toString().padStart(decimals, '0').replace(/0+$/, '')
  return (neg ? '-' : '') + s
}
function groupThousands(s) {
  return s.replace(/\B(?=(\d{3})+(?!\d))/g, ',')
}
export function parseMinor(input, decimals) {
  const s = String(input).trim()
  if (!s) return 0n
  const [whole, frac = ''] = s.split('.')
  const fracPadded = (frac + '0'.repeat(decimals)).slice(0, decimals)
  return BigInt(whole || '0') * 10n ** BigInt(decimals) + BigInt(fracPadded || '0')
}
export const parseXsu = (s) => parseMinor(s, XSU_DECIMALS)

export function shortHash(h, head = 10, tail = 6) {
  if (!h) return '—'
  if (h.length <= head + tail + 2) return h
  return `${h.slice(0, head)}…${h.slice(-tail)}`
}
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
