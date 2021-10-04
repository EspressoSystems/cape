pragma solidity ^0.8.0;

import "hardhat/console.sol";

library Rescue {

    uint256 internal constant N_ROUNDS = 12;
    uint256 internal constant STATE_SIZE = 4;
    uint256 internal constant PRIME = 21888242871839275222246405745257275088548364400416034343698204186575808495617;
    uint256 internal constant A = 5; // Dummy
    uint256 internal constant A_INV = 145457654212424242421; // Dummy

    function exp_mod(uint256 base, uint256 e, uint256 m) public view returns (uint256 o) {

        assembly {
        // define pointer
            let p := mload(0x40)
        // store data assembly-favouring ways
            mstore(p, 0x20)             // Length of Base
            mstore(add(p, 0x20), 0x20)  // Length of Exponent
            mstore(add(p, 0x40), 0x20)  // Length of Modulus
            mstore(add(p, 0x60), base)  // Base
            mstore(add(p, 0x80), e)     // Exponent
            mstore(add(p, 0xa0), m)     // Modulus
            if iszero(staticcall(sub(gas(), 2000), 0x05, p, 0xc0, p, 0x20)) {
                revert(0, 0)
            }
        // data
            o := mload(p)
        }
    }

    // Dummy version
    function add_vectors(uint256[STATE_SIZE] memory v1, uint256[STATE_SIZE] memory v2) internal returns (uint256[STATE_SIZE] memory){

        uint256[STATE_SIZE] memory v;

        for (uint j=0;j<STATE_SIZE;j++) {
            v[j] = addmod(v1[j], v2[j], PRIME);
        }

        return v;
    }

    // Dummy version
    function  linear_op(
        uint256[STATE_SIZE*STATE_SIZE] memory MDS,
        uint256[STATE_SIZE] memory v,
        uint256[STATE_SIZE] memory c
    ) private{
        uint256[STATE_SIZE] memory res;

        for (uint i=0; i<STATE_SIZE;i++) {
            uint256 sum = 0;
            for (uint j=0; j<STATE_SIZE;j++) {
                sum += MDS[i*STATE_SIZE+j] * v[j];
            }
            res[i] = res[i] + sum;
        }
        res = add_vectors(v,c);
    }


    // Adapted from Starkware implementation of rescue permutation function
    // https://etherscan.io/address/0x7B6fc6b18A20823c3d3663E58AB2Af8D780D0AFe#code#F3#L1
    function permutation_func(uint256[STATE_SIZE] memory elements)
    internal view
    returns (uint256[STATE_SIZE] memory)
    {
        uint256[STATE_SIZE*(N_ROUNDS+1) * 2] memory auxData;
        uint256 length = elements.length;

        uint256 prime_ = PRIME;
        uint256[] memory workingArea = new uint256[](length);
        for (uint256 i = 0; i < length; i++) {
            elements[i] = addmod(elements[i], auxData[i], prime_);
        }

        uint256 nRounds2 = N_ROUNDS * 2;
        uint256 a_inv_ = A_INV;
        for (uint256 round = 0; round < nRounds2; round++) {
            for (uint256 i = 0; i < STATE_SIZE; i++) {
                uint256 element = elements[i];
                if (round % 2 != 0) {
                    workingArea[i] = mulmod(mulmod(element, element, prime_), element, prime_);
                }
                else {
                    assembly {
                        function expmod(base, exponent, modulus) -> res {
                            let p := mload(0x40)
                            mstore(p, 0x20)                 // Length of Base.
                            mstore(add(p, 0x20), 0x20)      // Length of Exponent.
                            mstore(add(p, 0x40), 0x20)      // Length of Modulus.
                            mstore(add(p, 0x60), base)      // Base.
                            mstore(add(p, 0x80), exponent)  // Exponent.
                            mstore(add(p, 0xa0), modulus)   // Modulus.
                        // Call modexp precompile.
                            if iszero(staticcall(not(0), 0x05, p, 0xc0, p, 0x20)) {
                                revert(0, 0)
                            }
                            res := mload(p)
                        }
                        let position := add(workingArea, mul(add(i, 1), 0x20))
                        mstore(position, expmod(element, a_inv_, prime_))
                    }
                }
            }


            // To get the offset of the MDS matrix we need to skip auxData.length
            // and all the round constants.
            uint256 mdsByteOffset = 0x20 * (1 + length * (nRounds2 + 1));

            // MixLayer
            // elements = params.mds * workingArea
            assembly {
                let mdsRowPtr := add(auxData, mdsByteOffset)
                let stateSize := mul(length, 0x20)
                let workingAreaPtr := add(workingArea, 0x20)
                let statePtr := add(elements, 0x20)
                let mdsEnd := add(mdsRowPtr, mul(length, stateSize))

                for {} lt(mdsRowPtr, mdsEnd) { mdsRowPtr := add(mdsRowPtr, stateSize) } {
                    let sum := 0
                    for { let offset := 0} lt(offset, stateSize) { offset := add(offset, 0x20) } {
                        sum := addmod(
                        sum,
                        mulmod(mload(add(mdsRowPtr, offset)),
                        mload(add(workingAreaPtr, offset)),
                        prime_),
                        prime_)
                    }

                    mstore(statePtr, sum)
                    statePtr := add(statePtr, 0x20)
                }
            }

            for (uint256 i = 0; i < length; i++) {
                console.log(length * (round + 1) + i);
                elements[i] = addmod(elements[i], auxData[length * (round + 1) + i], prime_);
            }
        }

        return elements;
    }

    // Dummy version
    function  perm(uint256[STATE_SIZE] memory input) internal returns (uint256[STATE_SIZE] memory){

        uint256[STATE_SIZE*STATE_SIZE] memory MDS;
        uint256[STATE_SIZE] memory state;

        uint256[STATE_SIZE][2*N_ROUNDS+1] memory keys;

        state = add_vectors(keys[0],input);

        for (uint n_rounds=0;n_rounds < 2*N_ROUNDS+1;n_rounds++) {
            if (n_rounds %2 == 0){ // Pow
                // Pow
                for (uint j=0; j<STATE_SIZE;j++) {
                    state[j] = exp_mod(state[j],A, PRIME);
                }
            } else { // Pow inv
                for (uint j=0; j<STATE_SIZE;j++) {
                    state[j] = exp_mod(state[j],A_INV, PRIME);
                }
            }

            linear_op(MDS,state, keys[n_rounds]);
        }

        return state;
    }

    function hash(uint256 a, uint256 b, uint256 c, bool is_starkware)  internal returns (uint256[STATE_SIZE] memory){
        uint256[STATE_SIZE] memory input;
        uint256[STATE_SIZE] memory state;

        input[0] = a;
        input[1] = b;
        input[2] = c;
        input[3] = 0;

        if (is_starkware) {
            state = permutation_func(input);
        } else {
            state = perm(input);
        }

        return state;
    }
}