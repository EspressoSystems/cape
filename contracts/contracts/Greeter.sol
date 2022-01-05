//SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract Greeter {
    string private _greeting;

    constructor(string memory greeting) {
        console.log("Deploying a Greeter with _greeting:", greeting);
        _greeting = greeting;
    }

    function greet() public view returns (string memory) {
        return _greeting;
    }

    function setGreeting(string memory greeting) public {
        console.log("Changing _greeting from '%s' to '%s'", _greeting, greeting);
        _greeting = greeting;
    }
}
