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

contract Sponge {
    uint256 prime;
    uint256 r;
    uint256 c;
    uint256 m;
    uint256 outputSize;
    uint256 nRounds;

    constructor(
        uint256 prime_,
        uint256 r_,
        uint256 c_,
        uint256 nRounds_
    ) public {
        prime = prime_;
        r = r_;
        c = c_;
        m = r + c;
        outputSize = c;
        nRounds = nRounds_;
    }

    function LoadAuxdata()
        internal
        view
        returns (
            uint256[] memory /*auxdata*/
        );

    function permutation_func(
        uint256[] memory, /*auxdata*/
        uint256[] memory /*elements*/
    )
        internal
        view
        returns (
            uint256[] memory /*hash_elements*/
        );

    function sponge(uint256[] memory inputs)
        internal
        view
        returns (uint256[] memory outputElements)
    {
        uint256 inputLength = inputs.length;
        for (uint256 i = 0; i < inputLength; i++) {
            require(inputs[i] < prime, "elements do not belong to the field");
        }

        require(inputLength % r == 0, "Number of field elements is not divisible by r.");

        uint256[] memory state = new uint256[](m);
        for (uint256 i = 0; i < m; i++) {
            state[i] = 0; // fieldZero.
        }

        uint256[] memory auxData = LoadAuxdata();
        uint256 n_columns = inputLength / r;
        for (uint256 i = 0; i < n_columns; i++) {
            for (uint256 j = 0; j < r; j++) {
                state[j] = addmod(state[j], inputs[i * r + j], prime);
            }
            state = permutation_func(auxData, state);
        }

        require(outputSize <= r, "No support for more than r output elements.");
        outputElements = new uint256[](outputSize);
        for (uint256 i = 0; i < outputSize; i++) {
            outputElements[i] = state[i];
        }
    }

    function getParameters() public view returns (uint256[] memory status) {
        status = new uint256[](4);
        status[0] = prime;
        status[1] = r;
        status[2] = c;
        status[3] = nRounds;
    }
}
