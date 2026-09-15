import hmac, hashlib
def hkdf(ikm, salt, info, L=32):
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm, t = b"", b""
    i = 1
    while len(okm) < L:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        okm += t; i += 1
    return okm[:L]
out = hkdf(b"top secret key material", b"safenet-sentinel-reveal-salt", b"request-1")
print(out.hex())
print(out.hex() == "de66ad87d39718318f7ec36177e9e2286b5c0ade3dc0de22b65e9ee55ccaab0d")
