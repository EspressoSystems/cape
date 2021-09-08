//SPDX-License-Identifier: Unlicense
pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import "hardhat/console.sol";

contract Greeter {
    string private greeting;

    constructor(string memory _greeting) {
        console.log("Deploying a Greeter with greeting:", _greeting);
        greeting = _greeting;
    }

    function greet() public view returns (string memory) {
        return greeting;
    }

    function setGreeting(string memory _greeting) public {

        console.log("Changing greeting from '%s' to '%s'", greeting, _greeting);
        greeting = _greeting;
    }

    function addG1() public view returns (G1Point memory) {
        uint8 G1_ADD = 0xa;
        uint gas = 15000;

        G1Point memory generator_g1 = G1Point(Fp(0,0) , Fp(0,0));
        G1Point memory p2 = G1Point(Fp(0,0) , Fp(0,0));
        G1Point memory g_plus_g = g1Add(generator_g1, generator_g1, G1_ADD,gas);
        return generator_g1;
    }

    /////// BLS12-381 group addition ////////////////////////////////////////////
    // Fp is a field element with the high-order part stored in `a`.
    struct Fp {
        uint256 a;
        uint256 b;
    }

    // G1Point represents a point on BLS12-377 over Fp with coordinates (X,Y);
    struct G1Point {
        Fp X;
        Fp Y;
    }

    function g1Add(
        G1Point memory a,
        G1Point memory b,
        uint8 precompile,
        uint256 gasEstimate
    ) internal view returns (G1Point memory c) {
        uint256[8] memory input;
        input[0] = a.X.a;
        input[1] = a.X.b;
        input[2] = a.Y.a;
        input[3] = a.Y.b;

        input[4] = b.X.a;
        input[5] = b.X.b;
        input[6] = b.Y.a;
        input[7] = b.Y.b;

        bool success;
        assembly {
            success := staticcall(gasEstimate, precompile, input, 256, input, 128)
        // deallocate the input, leaving dirty memory
            mstore(0x40, input)
        }

        require(success, "g1 add precompile failed");
        c.X.a = input[0];
        c.X.b = input[1];
        c.Y.a = input[2];
        c.Y.b = input[3];
    }

}
