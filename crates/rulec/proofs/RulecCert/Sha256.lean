/-
  SHA-256, so that the checker can say **which text** a certificate is about.

  A certificate carries the digest of the `.rule` file it was made from. Without a digest
  of its own the checker would have to take that on trust, and "the claims hold" would be a
  statement about no particular rule. FIPS 180-4; nothing here is proved, and nothing needs
  to be — a wrong digest can only make the checker refuse.
-/

namespace RulecCert.Sha256

private def K : Array UInt32 := #[
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2]

private def rotr (x : UInt32) (n : UInt32) : UInt32 := (x >>> n) ||| (x <<< (32 - n))

private def hex8 (x : UInt32) : String :=
  let d := fun (n : UInt32) => "0123456789abcdef".toList[(n &&& 15).toNat]!
  String.ofList [d (x >>> 28), d (x >>> 24), d (x >>> 20), d (x >>> 16),
                 d (x >>> 12), d (x >>> 8), d (x >>> 4), d x]

/-- The digest of a byte string, as the 64 lower-case hex characters a certificate uses. -/
def hex (bs : ByteArray) : String := Id.run do
  let bits : UInt64 := bs.size.toUInt64 * 8
  let mut m := bs.push 0x80
  while m.size % 64 != 56 do
    m := m.push 0
  for i in [0:8] do
    m := m.push ((bits >>> (56 - 8 * i).toUInt64).toUInt8)
  let mut h : Array UInt32 :=
    #[0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]
  for blk in [0 : m.size / 64] do
    let off := blk * 64
    let mut w : Array UInt32 := Array.replicate 64 0
    for i in [0:16] do
      w := w.set! i ((m[off + 4 * i]!.toUInt32 <<< 24) ||| (m[off + 4 * i + 1]!.toUInt32 <<< 16)
        ||| (m[off + 4 * i + 2]!.toUInt32 <<< 8) ||| m[off + 4 * i + 3]!.toUInt32)
    for i in [16:64] do
      let x := w[i - 15]!
      let y := w[i - 2]!
      let s0 := (rotr x 7) ^^^ (rotr x 18) ^^^ (x >>> 3)
      let s1 := (rotr y 17) ^^^ (rotr y 19) ^^^ (y >>> 10)
      w := w.set! i (w[i - 16]! + s0 + w[i - 7]! + s1)
    let mut a := h[0]!
    let mut b := h[1]!
    let mut c := h[2]!
    let mut d := h[3]!
    let mut e := h[4]!
    let mut f := h[5]!
    let mut g := h[6]!
    let mut hh := h[7]!
    for i in [0:64] do
      let s1 := (rotr e 6) ^^^ (rotr e 11) ^^^ (rotr e 25)
      let ch := (e &&& f) ^^^ ((~~~e) &&& g)
      let t1 := hh + s1 + ch + K[i]! + w[i]!
      let s0 := (rotr a 2) ^^^ (rotr a 13) ^^^ (rotr a 22)
      let maj := (a &&& b) ^^^ (a &&& c) ^^^ (b &&& c)
      let t2 := s0 + maj
      hh := g; g := f; f := e; e := d + t1; d := c; c := b; b := a; a := t1 + t2
    h := #[h[0]! + a, h[1]! + b, h[2]! + c, h[3]! + d, h[4]! + e, h[5]! + f, h[6]! + g, h[7]! + hh]
  return String.join (h.toList.map hex8)

end RulecCert.Sha256
