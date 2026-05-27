// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Math} from "@oz/utils/math/Math.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @dev Stateless FROST math helpers for test contracts, parameterised by participant
///      addresses so they are decoupled from any specific `ParticipantMerkleTree` instance.
library FROSTMath {
    /// @dev Evaluate f(x) = a[0] + a[1]·x + … at the FROST identifier of `participant`.
    function evalPolynomial(uint256[] memory a, address participant) internal view returns (uint256 r) {
        r = a[0];
        uint256 x = FROST.identifier(participant);
        uint256 xx = 1;
        for (uint256 j = 1; j < a.length; j++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = addmod(r, mulmod(a[j], xx, Secp256k1.N), Secp256k1.N);
        }
    }

    /// @dev Evaluate C(x) = c[0] + c[1]·x + … (EC points) at the FROST identifier of `participant`.
    function evalCommitmentPolynomial(ForgeSecp256k1.P[] memory c, address participant)
        internal
        returns (ForgeSecp256k1.P memory r)
    {
        r = c[0];
        uint256 x = FROST.identifier(participant);
        uint256 xx = 1;
        for (uint256 j = 1; j < c.length; j++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = ForgeSecp256k1.add(r, ForgeSecp256k1.mul(xx, c[j]));
        }
    }

    /// @dev Compute the Lagrange coefficient λ_i for `signer` over the set `signers`.
    function lagrangeCoefficient(address[] memory signers, address signer) internal view returns (uint256 lambda) {
        uint256 numerator = 1;
        uint256 denominator = 1;
        uint256 minusId = Secp256k1.N - FROST.identifier(signer);
        for (uint256 j = 0; j < signers.length; j++) {
            if (signers[j] == signer) continue;
            uint256 x = FROST.identifier(signers[j]);
            numerator = mulmod(numerator, x, Secp256k1.N);
            denominator = mulmod(denominator, addmod(x, minusId, Secp256k1.N), Secp256k1.N);
        }
        return mulmod(numerator, Math.invModPrime(denominator, Secp256k1.N), Secp256k1.N);
    }

    /// @dev ECDH: encrypt/decrypt `x` with private key `k` and public key `q`.
    function ecdh(uint256 x, uint256 k, ForgeSecp256k1.P memory q) internal returns (uint256) {
        return x ^ ForgeSecp256k1.toPoint(ForgeSecp256k1.mul(k, q)).x;
    }
}
