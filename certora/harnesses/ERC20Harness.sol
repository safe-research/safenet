// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "../../contracts/lib/openzeppelin-contracts/contracts/token/ERC20/ERC20.sol";

contract ERC20Harness is ERC20 {
    constructor(string memory _name, string memory _symbol)
        ERC20(_name, _symbol)
    {}
}
