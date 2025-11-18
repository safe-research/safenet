// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract Secp256k1Test is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    function test_Add() public {
        ForgeSecp256k1.P memory w = ForgeSecp256k1.rand();
        ForgeSecp256k1.P memory v = ForgeSecp256k1.rand();

        Secp256k1.Point memory p = w.toPoint();
        Secp256k1.Point memory q = v.toPoint();
        Secp256k1.Point memory r = ForgeSecp256k1.add(w, v).toPoint();

        _assertEqPoint(r, Secp256k1.add(p, q));
    }

    function test_AddZero() public {
        Secp256k1.Point memory p = ForgeSecp256k1.rand().toPoint();
        Secp256k1.Point memory z;

        _assertEqPoint(p, Secp256k1.add(p, z));
        _assertEqPoint(p, Secp256k1.add(z, p));
        _assertEqPoint(z, Secp256k1.add(z, z));
    }

    function test_AddDouble() public {
        ForgeSecp256k1.P memory w = ForgeSecp256k1.rand();

        Secp256k1.Point memory p = w.toPoint();
        Secp256k1.Point memory r = ForgeSecp256k1.mul(2, w).toPoint();

        _assertEqPoint(r, Secp256k1.add(p, p));
    }

    function test_AddNegative() public {
        ForgeSecp256k1.P memory w = ForgeSecp256k1.rand();

        Secp256k1.Point memory p = w.toPoint();
        Secp256k1.Point memory minusP = w.neg().toPoint();
        Secp256k1.Point memory z;

        _assertEqPoint(z, Secp256k1.add(p, minusP));
    }

    function test_MulMulAdd() public {
        uint256 z = vm.randomUint();
        uint256 e = vm.randomUint();
        ForgeSecp256k1.P memory w = ForgeSecp256k1.rand();

        Secp256k1.Point memory p = w.toPoint();
        Secp256k1.Point memory r = ForgeSecp256k1.add(ForgeSecp256k1.g(z), ForgeSecp256k1.mul(e, w.neg())).toPoint();

        Secp256k1.mulmuladd(z, e, p, r);
    }

    function test_MulMulAddIdentity() public {
        Secp256k1.Point memory p = ForgeSecp256k1.rand().toPoint();

        Secp256k1.mulmuladd(0, Secp256k1.N - 1, p, p);
    }

    function test_NotOnCurve() public {
        Secp256k1.Point memory p = Secp256k1.Point({x: 1, y: 0});
        Secp256k1.Point memory z;

        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callAdd(p, z);
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callAdd(z, p);
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callMulMulAdd(0, 0, p, z);
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callMulMulAdd(0, 0, z, p);
    }

    function test_InvalidMulMulAddWitness() public {
        Secp256k1.Point memory p = ForgeSecp256k1.rand().toPoint();

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        this.callMulMulAdd(0, 1, p, p); // `0⋅G - 1⋅P = -P != P`

        // The `ecrecover` trick only works for X-coordinates smaller than the
        // curve order. Demonstrate that the `mulmuladd` fails despite being
        // valid for these specific kinds of points.
        p = Secp256k1.Point({
            x: 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141,
            y: 0x98f66641cb0ae1776b463ebdee3d77fe2658f021db48e2c8ac7ab4c92f83621e
        });
        assertTrue(p.x >= Secp256k1.N);

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        this.callMulMulAdd(0, Secp256k1.N - 1, p, p);
    }

    function callAdd(Secp256k1.Point memory p, Secp256k1.Point memory q)
        external
        view
        returns (Secp256k1.Point memory r)
    {
        return Secp256k1.add(p, q);
    }

    function callMulMulAdd(uint256 z, uint256 e, Secp256k1.Point memory p, Secp256k1.Point memory r) external view {
        Secp256k1.mulmuladd(z, e, p, r);
    }

    function _assertEqPoint(Secp256k1.Point memory p, Secp256k1.Point memory q) private pure {
        assertEq(p.x, q.x);
        assertEq(p.y, q.y);
    }
}
