// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "@oz/token/ERC20/ERC20.sol";
import {Ownable} from "@oz/access/Ownable.sol";
import {ERC20Permit} from "@oz/token/ERC20/extensions/ERC20Permit.sol";

contract MyToken is ERC20, Ownable, ERC20Permit {
    constructor(address owner) ERC20("MyToken", "MTK") Ownable(owner) ERC20Permit("MyToken") {
        _mint(owner, 100_000 * 10 ** decimals());
    }

    function mint(address to, uint256 amount) public onlyOwner {
        _mint(to, amount);
    }
}
