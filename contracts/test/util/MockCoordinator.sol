// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract MockCoordinator {
    mapping(FROSTGroupId.T => Secp256k1.Point) groupKeys;

    function setGroupKey(FROSTGroupId.T group, Secp256k1.Point memory key) external {
        groupKeys[group] = key;
    }

    function groupKey(FROSTGroupId.T group) external view returns (Secp256k1.Point memory key) {
        return groupKeys[group];
    }

    function signatureVerify(FROSTSignatureId.T, FROSTGroupId.T, bytes32)
        external
        view
        returns (FROST.Signature memory signature)
    {}
}
