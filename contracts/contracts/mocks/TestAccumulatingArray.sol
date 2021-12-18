//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import {AccumulatingArray} from "../libraries/AccumulatingArray.sol";

contract TestAccumulatingArray {
    using AccumulatingArray for AccumulatingArray.Data;

    function accumulate(uint256[][] memory arrays, uint256 maxLength)
        public
        pure
        returns (uint256[] memory)
    {
        AccumulatingArray.Data memory accumulated = AccumulatingArray.create(
            maxLength
        );
        for (uint256 i = 0; i < arrays.length; i++) {
            accumulated.add(arrays[i]);
        }
        return accumulated.toArray();
    }

    // Adds single element arrays as individual items
    function accumulateWithIndividuals(
        uint256[][] memory arrays,
        uint256 maxLength
    ) public pure returns (uint256[] memory) {
        AccumulatingArray.Data memory accumulated = AccumulatingArray.create(
            maxLength
        );
        for (uint256 i = 0; i < arrays.length; i++) {
            if (arrays[i].length == 1) {
                accumulated.add(arrays[i][0]);
            } else {
                accumulated.add(arrays[i]);
            }
        }
        return accumulated.toArray();
    }
}
