// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {BN254} from "../libraries/BN254.sol";
import {PlonkVerifier as V} from "../verifier/PlonkVerifier.sol";
import {PolynomialEval} from "../libraries/PolynomialEval.sol";

contract TestPlonkVerifier is V {
    function computeAlphaPowers(uint256 alpha)
        public
        pure
        returns (uint256[2] memory alphaPowers)
    {
        return V._computeAlphaPowers(alpha);
    }
}
