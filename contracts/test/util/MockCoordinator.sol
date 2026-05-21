// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract MockCoordinator {
    mapping(FROSTGroupId.T => Secp256k1.Point) groupKeys;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTSignatureId.T => bool) private $rejectedSignatures;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTSignatureId.T => bytes32) private $signatureMessages;

    function setGroupKey(FROSTGroupId.T group, Secp256k1.Point memory key) external {
        groupKeys[group] = key;
    }

    function setSignRejected(FROSTSignatureId.T sid, bool rejected) external {
        $rejectedSignatures[sid] = rejected;
    }

    function setSignatureMessage(FROSTSignatureId.T sid, bytes32 message) external {
        $signatureMessages[sid] = message;
    }

    function groupKey(FROSTGroupId.T group) external view returns (Secp256k1.Point memory key) {
        return groupKeys[group];
    }

    function sign(FROSTGroupId.T, bytes32) external pure returns (FROSTSignatureId.T sid) {}

    function signatureVerify(FROSTSignatureId.T, FROSTGroupId.T, bytes32)
        external
        pure
        returns (FROST.Signature memory signature)
    {}

    function signatureValue(FROSTSignatureId.T) external pure returns (FROST.Signature memory) {}

    function isSignRejected(FROSTSignatureId.T sid) external view returns (bool) {
        return $rejectedSignatures[sid];
    }

    function signatureMessage(FROSTSignatureId.T sid) external view returns (bytes32) {
        return $signatureMessages[sid];
    }
}
