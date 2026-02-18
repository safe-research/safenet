// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SafeSingletonFactory} from "./util/SafeSingletonFactory.sol";
import {ERC20} from "@oz/token/ERC20/ERC20.sol";
import {Ownable} from "@oz/access/Ownable.sol";
import {ERC20Permit} from "@oz/token/ERC20/extensions/ERC20Permit.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

contract MyToken is ERC20, Ownable, ERC20Permit {
    constructor(address owner) ERC20("MyToken", "MTK") Ownable(owner) ERC20Permit("MyToken") {
        _mint(owner, 100000 * 10 ** decimals());
    }

    function mint(address to, uint256 amount) public onlyOwner {
        _mint(to, amount);
    }
}

contract ERC20Script is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function setUp() public {}

    function run() public returns (MyToken erc20) {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");
        uint256 factoryChoice = vm.envUint("FACTORY");

        vm.startBroadcast(privateKey);

        if (factoryChoice == 1) {
            // Deploy the Staking contract using the SafeSingletonFactory
            erc20 = MyToken(
                SafeSingletonFactory.deploy({
                    salt: bytes32(0), code: abi.encodePacked(type(MyToken).creationCode, abi.encode(msg.sender))
                })
            );
        } else if (factoryChoice == 2) {
            // Deploy the Staking contract using the DeterministicDeployment factory
            erc20 = MyToken(
                DeterministicDeployment.CANONICAL
                    .deployWithArgs(bytes32(0), type(MyToken).creationCode, abi.encode(msg.sender))
            );
        } else {
            revert("Invalid FACTORY choice");
        }

        vm.stopBroadcast();

        console.log("ERC20 deployed at:", address(erc20));
    }
}
