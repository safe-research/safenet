// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/// @title Secp256k1
/// @notice Secp256k1 curve operations.
library Secp256k1 {
    struct Point {
        uint256 x;
        uint256 y;
    }

    error NotOnCurve();
    error InvalidMulMulAddWitness();

    uint256 internal constant B = 7;
    uint256 internal constant P = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f;
    uint256 internal constant N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141;

    /// @notice Computes `R = P + Q`.
    function add(Point memory p, Point memory q) internal view returns (Point memory r) {
        // See <https://www.hyperelliptic.org/EFD/g1p/auto-shortw.html>
        //
        // Since we are doing a single addition per transaction; we just
        // implement the affine formulas instead of the projective formulas
        // (which are typically considered more efficient when doing multiple
        // point additions at a time). The additional gas cost of storing the
        // projective `Z` coordinate in storage is higher than the additional
        // cost of the extra arithmetic from using affine coordinates. Note that
        // we optimize the computation to do as few `_divmod`s as possible (one
        // per operation), as that is by far the most expensive computation
        // (~200 gas for the `modexp` precompile call).

        (uint256 px, uint256 py) = _unpack(p);
        (uint256 qx, uint256 qy) = _unpack(q);

        uint256 l;
        if (px | py == 0) {
            // `P` is the point at infinity, and `0 + Q = Q`.
            return q;
        } else if (qx | qy == 0) {
            // `Q` is the point at infinity, and `P + 0 = P`.
            return p;
        } else if (px != qx) {
            // Point addition, compute the slope through `P` and `Q`:
            //     λ = (Qy - Py) / (Qx - Px)
            unchecked {
                l = _divmod(addmod(qy, P - py, P), addmod(qx, P - px, P), P);
            }
        } else if (py == qy) {
            // Point doubling, compute the slope of the tangent at `P`:
            //     λ = (3⋅Px² + a) / (2⋅Py)
            //       = (3⋅Px²) / (2⋅Py)
            // Noting that `a = 0` for the `secp256k1` curve.
            unchecked {
                l = _divmod(mulmod(3, mulmod(px, px, P), P), addmod(py, py, P), P);
            }
        } else {
            // This branch happens iff `P = -P` in which case their sum is the
            // point at infinity (represented by the 0-value) that `r` is
            // initialized with.
            return r;
        }

        // Compute the coordinates for the point `R`:
        //     Rx = λ² - Px - Qx
        //        = λ² - (Px + Qx)
        //     Ry = (2⋅Px + Qx)⋅λ - λ³ - Py
        //        = (2⋅Px + Qx - λ²)⋅λ - Py
        //        = (Px - (λ² - (Px + Qx)))⋅λ - Py
        //        = (Px - Rx)⋅λ - Py
        // Noting that `Px = Qx` for point doubling.
        unchecked {
            r.x = addmod(mulmod(l, l, P), P - addmod(px, qx, P), P);
            r.y = addmod(mulmod(addmod(px, P - r.x, P), l, P), P - py, P);
        }
        return r;
    }

    /// @notice Verifies that `z⋅G - e⋅P = R`.
    function mulmuladd(uint256 z, uint256 e, Point memory p, Point memory r) internal view {
        // See <https://ethresear.ch/t/you-can-kinda-abuse-ecrecover-to-do-ecmul-in-secp256k1-today/2384>
        //
        // This function uses a trick to abuse the `ecrecover` precompile in
        // order to compute a mul-mul-add operation of `-z` times the curve
        // generator point plus `e` time the point `P` defined by the
        // coordinates `Px` and `Py`. The caveat with this trick is that it
        // doesn't return the resulting point, but a public address (which is a
        // truncated hash of the resulting point's coordinates, and why we
        // require a witness `r` instead of just returning a result).
        // Additionally, it only supports points `P` with x-coordinates that
        // are elements in Fn. In practice (assuming uniform distribution of
        // x-coordinates), this happens for roughly one in every 3.7e39 points
        // and is negligeable.

        (uint256 px, uint256 py) = _unpack(p);
        (uint256 rx, uint256 ry) = _unpack(r);
        bool valid;
        assembly ("memory-safe") {
            // Perform `ecrecover(z * Px, v, Px, e * Px) = address(-R)`
            let ptr := mload(0x40)
            mstore(ptr, mulmod(z, px, N))
            mstore(add(ptr, 0x20), add(and(py, 1), 27))
            mstore(add(ptr, 0x40), px)
            mstore(add(ptr, 0x60), mulmod(e, px, N))
            let minusR :=
                mul(mload(0x00), and(eq(returndatasize(), 0x20), staticcall(gas(), 0x1, ptr, 0x80, 0x00, 0x20)))

            // Compute `address(-R)` from the provided witness `r` and check it
            // matches the result of the `ecrecover` operation.
            mstore(0x00, rx)
            mstore(0x20, sub(P, ry))
            valid := eq(minusR, and(keccak256(0x00, 0x40), 0xffffffffffffffffffffffffffffffffffffffff))
        }
        require(valid, InvalidMulMulAddWitness());
    }

    function _unpack(Point memory p) private pure returns (uint256 x, uint256 y) {
        x = p.x;
        y = p.y;
        bool valid;
        assembly ("memory-safe") {
            // Check (branchlessly) that `p` is either the point at infinity,
            // which is encoded as the 0-value, or that the point's coordinates
            // satisfy the curve equasion:
            //      Py² = Px³ + b
            // And that both `Px` and `Py` are elements in Fp.
            valid := or(
                iszero(or(x, y)),
                and(eq(mulmod(y, y, P), addmod(mulmod(x, mulmod(x, x, P), P), B, P)), and(lt(x, P), lt(y, P)))
            )
        }
        require(valid, NotOnCurve());
    }

    function _divmod(uint256 x, uint256 y, uint256 p) private view returns (uint256 result) {
        // Division in a prime field is defined as multiplication by the
        // divisor's multiplicative inverse. The most efficient way to compute
        // a multiplicative inverse on the EVM is by applying Fermat's little
        // theorem, which implies that `inv(y) = y ** (p - 2)  (mod p)` is the
        // multiplicative inverse of `y` (where `inv(y) * y = 1  (mod p)`). We
        // use the `modexp (0x05)` precompile for computing this. The use of
        // assembly is required, as there is no `modexp` built-in in Solidity.
        bool success;
        assembly ("memory-safe") {
            // Call `modexp(32, 32, 32, y, p - 2, p)`, where 32 is the size in
            // bytes of the base, exponent, and modulo. The result is `inv(y)`,
            // the multiplicative inverse of `y`.
            let ptr := mload(0x40)
            mstore(ptr, 0x20)
            mstore(add(ptr, 0x20), 0x20)
            mstore(add(ptr, 0x40), 0x20)
            mstore(add(ptr, 0x60), y)
            mstore(add(ptr, 0x80), sub(p, 2))
            mstore(add(ptr, 0xa0), p)
            success := and(eq(returndatasize(), 0x20), staticcall(gas(), 0x5, ptr, 0xc0, 0x00, 0x20))

            // Compute `x / y = x * inv(y)`.
            result := mulmod(x, mload(0x00), p)
        }
        assert(success);
    }
}
