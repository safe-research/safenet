# run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): F-CORE-038 re-check without a toolchain
# dependency. Re-derives the reference vector of crates/core/src/kdf.rs and executes the two
# collision properties the finding and its run-2 nuance describe, using RFC 5869 HKDF-SHA256 with
# the domain as salt and the message parts as info (the shape of kdf::derive_key).
import hashlib
import hmac


def hkdf(ikm, salt, info, length=32):
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm, block, counter = b"", b"", 1
    while len(okm) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        okm += block
        counter += 1
    return okm[:length]


def derive_key(ikm, domain, parts):
    # expand_multi_info feeds the parts to HMAC one after another: plain concatenation.
    return hkdf(ikm, domain, b"".join(parts))


ikm = b"top secret key material"
vector = derive_key(ikm, b"safenet-sentinel-reveal-salt", [b"request-1"])
print("reference vector:", vector.hex())
print("matches kdf.rs literal:", vector.hex() == "de66ad87d39718318f7ec36177e9e2286b5c0ade3dc0de22b65e9ee55ccaab0d")

# 1. Multi-part info is ambiguous: two different part splits derive the same key.
a = derive_key(ikm, b"domain-a", [b"ab", b"c"])
b = derive_key(ikm, b"domain-a", [b"a", b"bc"])
print("['ab','c'] == ['a','bc']:", a == b)

# 2. Run-2 nuance: HMAC pads short keys with zero bytes, so a domain and the same domain with
#    trailing zero bytes are the same salt; "different domains can never collide" is overstated.
c = derive_key(ikm, b"domain-a", [b"request-1"])
d = derive_key(ikm, b"domain-a\x00", [b"request-1"])
print("domain 'domain-a' == 'domain-a\\x00':", c == d)

# 3. Run-2 nuance: a domain longer than the SHA-256 block (64 bytes) is hashed first, so it
#    collides with its own digest used as the domain.
long_domain = b"x" * 65
e = derive_key(ikm, long_domain, [b"request-1"])
f = derive_key(ikm, hashlib.sha256(long_domain).digest(), [b"request-1"])
print("65-byte domain == sha256(domain):", e == f)
