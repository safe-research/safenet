// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title Forge Secp256k1
/// @dev The Forge standard library doesn't provide any direct methods for doing
///      elliptic curve math on the `secp256k1` curve, BUT, it does provide a
///      `Wallet` API that takes a private key and returns a public key. This
///      allows us to create a reference `secp256k1` implementation.
library ForgeSecp256k1 {
    Vm private constant _VM = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    struct P {
        Vm.Wallet w;
    }

    function g(uint256 k) internal returns (P memory r) {
        return P({w: _VM.createWallet(k)});
    }

    function rand() internal returns (P memory r) {
        // The `ecrecover` trick only works for x-coordinates smaller than the
        // curve order. Assuming uniform distribution of x-coordinates for curve
        // points, this occurs once in every 3.7e39, so this loop will
        // practically always run exactly once.
        while (true) {
            r = g(_VM.randomUint());
            if (r.w.publicKeyX < Secp256k1.N) {
                return r;
            }
        }
        assert(false);
    }

    function add(P memory p, P memory q) internal returns (P memory r) {
        return g(addmod(p.w.privateKey, q.w.privateKey, Secp256k1.N));
    }

    function neg(P memory p) internal returns (P memory r) {
        return g(Secp256k1.N - p.w.privateKey);
    }

    function mul(uint256 a, P memory p) internal returns (P memory r) {
        return g(mulmod(a, p.w.privateKey, Secp256k1.N));
    }

    function toPoint(P memory p) internal pure returns (Secp256k1.Point memory r) {
        r.x = p.w.publicKeyX;
        r.y = p.w.publicKeyY;
    }
}
