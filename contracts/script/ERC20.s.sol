// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {ERC20} from "@oz/token/ERC20/ERC20.sol";
import {Ownable} from "@oz/access/Ownable.sol";
import {ERC20Permit} from "@oz/token/ERC20/extensions/ERC20Permit.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

contract MyToken is ERC20, Ownable, ERC20Permit {
    constructor(address owner) ERC20("MyToken", "MTK") Ownable(owner) ERC20Permit("MyToken") {
        _mint(owner, 100_000 * 10 ** decimals());
    }

    function mint(address to, uint256 amount) public onlyOwner {
        _mint(to, amount);
    }
}

contract DeployERC20Script is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address erc20) {
        uint256 factoryChoice = vm.envUint("FACTORY");
        DeterministicDeployment.Factory factory;

        if (factoryChoice == 1) {
            factory = DeterministicDeployment.SAFE_SINGLETON_FACTORY;
        } else if (factoryChoice == 2) {
            factory = DeterministicDeployment.CANONICAL;
        } else {
            revert("Invalid FACTORY choice");
        }

        vm.startBroadcast();

        erc20 = factory.deployWithArgs(bytes32(0), type(MyToken).creationCode, abi.encode(msg.sender));

        vm.stopBroadcast();

        console.log("ERC20 deployed at:", address(erc20));
    }
}
