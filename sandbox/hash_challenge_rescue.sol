/*
    This smart contract was written by StarkWare Industries Ltd. as part of the STARK-friendly hash
    challenge effort, funded by the Ethereum Foundation.
    The contract will pay out 8 ETH to the first finder of a collision in Rescue with rate 10
    and capacity 4 at security level of 256 bits, if such a collision is discovered before the end
    of March 2020.
    More information about the STARK-friendly hash challenge can be found
    here https://starkware.co/hash-challenge/.
    More information about the STARK-friendly hash selection process (of which this challenge is a
    part) can be found here
    https://medium.com/starkware/stark-friendly-hash-tire-kicking-8087e8d9a246.
    Sage code reference implementation for the contender hash functions available
    at https://starkware.co/hash-challenge-implementation-reference-code/.
*/

/*
  Copyright 2019 StarkWare Industries Ltd.

  Licensed under the Apache License, Version 2.0 (the "License").
  You may not use this file except in compliance with the License.
  You may obtain a copy of the License at

  https://www.starkware.co/open-source-license/

  Unless required by applicable law or agreed to in writing,
  software distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions
  and limitations under the License.
*/

pragma solidity ^0.5.2;

import "./Base.sol";
import "./Sponge.sol";

contract STARK_Friendly_Hash_Challenge_Rescue_S256b is Base, Sponge {
    uint256 MAX_CONSTANTS_PER_CONTRACT = 768;

    address roundConstantsContract;
    address mdsContract;
    uint256 inv3;

    constructor(
        uint256 prime,
        uint256 r,
        uint256 c,
        uint256 nRounds,
        uint256 inv3_,
        address roundConstantsContract_,
        address mdsContract_
    ) public payable Sponge(prime, r, c, nRounds) {
        inv3 = inv3_;
        roundConstantsContract = roundConstantsContract_;
        mdsContract = mdsContract_;
    }

    function LoadAuxdata() internal view returns (uint256[] memory auxData) {
        uint256 round_constants = m * (2 * nRounds + 1);
        require(
            round_constants <= MAX_CONSTANTS_PER_CONTRACT,
            "The code supports up to one roundConstantsContracts."
        );

        uint256 mdsSize = m * m;
        auxData = new uint256[](round_constants + mdsSize);

        address roundsContractAddr = roundConstantsContract;
        address mdsContractAddr = mdsContract;

        assembly {
            let offset := add(auxData, 0x20)
            let roundConstantsLength := mul(round_constants, 0x20)
            extcodecopy(roundsContractAddr, offset, 0, roundConstantsLength)
            offset := add(offset, roundConstantsLength)
            extcodecopy(mdsContractAddr, offset, 0, mul(mdsSize, 0x20))
        }
    }

    function permutation_func(uint256[] memory auxData, uint256[] memory elements)
        internal
        view
        returns (uint256[] memory)
    {
        uint256 length = elements.length;
        require(length == m, "elements length is not equal to m.");

        uint256 prime_ = prime;
        uint256[] memory workingArea = new uint256[](length);
        for (uint256 i = 0; i < length; i++) {
            elements[i] = addmod(elements[i], auxData[i], prime_);
        }

        uint256 nRounds2 = nRounds * 2;
        uint256 inv3_ = inv3;
        for (uint256 round = 0; round < nRounds2; round++) {
            for (uint256 i = 0; i < m; i++) {
                uint256 element = elements[i];
                if (round % 2 != 0) {
                    workingArea[i] = mulmod(mulmod(element, element, prime_), element, prime_);
                } else {
                    assembly {
                        function expmod(base, exponent, modulus) -> res {
                            let p := mload(0x40)
                            mstore(p, 0x20) // Length of Base.
                            mstore(add(p, 0x20), 0x20) // Length of Exponent.
                            mstore(add(p, 0x40), 0x20) // Length of Modulus.
                            mstore(add(p, 0x60), base) // Base.
                            mstore(add(p, 0x80), exponent) // Exponent.
                            mstore(add(p, 0xa0), modulus) // Modulus.
                            // Call modexp precompile.
                            if iszero(staticcall(not(0), 0x05, p, 0xc0, p, 0x20)) {
                                revert(0, 0)
                            }
                            res := mload(p)
                        }
                        let position := add(workingArea, mul(add(i, 1), 0x20))
                        mstore(position, expmod(element, inv3_, prime_))
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

                for {

                } lt(mdsRowPtr, mdsEnd) {
                    mdsRowPtr := add(mdsRowPtr, stateSize)
                } {
                    let sum := 0
                    for {
                        let offset := 0
                    } lt(offset, stateSize) {
                        offset := add(offset, 0x20)
                    } {
                        sum := addmod(
                            sum,
                            mulmod(
                                mload(add(mdsRowPtr, offset)),
                                mload(add(workingAreaPtr, offset)),
                                prime_
                            ),
                            prime_
                        )
                    }

                    mstore(statePtr, sum)
                    statePtr := add(statePtr, 0x20)
                }
            }

            for (uint256 i = 0; i < length; i++) {
                elements[i] = addmod(elements[i], auxData[length * (round + 1) + i], prime_);
            }
        }

        return elements;
    }
}
