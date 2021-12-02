//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import {Curve} from "./BN254.sol";
import "hardhat/console.sol";

contract PlonkVerifier {
    string private greeting;

    constructor() {
        //TODO
    }

    function batchVerify(bytes memory chunk) internal returns (bool) {
        // Count the number of transactions
        uint256 captxSize = 3000;
        uint256 nCaptx = chunk.length / captxSize;

        // We lower bound the complexity by
        // 1 pairing check
        // 2  multi exp in G1 of size $nCaptx$ (See rust code PlonkKzgSnark.batch_verify)
        // Cost of preparePcsInfo(nCaptx)

        // 2 multi exp in G1 of size $nCaptx$
        runMultiExpG1(nCaptx);
        runMultiExpG1(nCaptx);

        // 1 pairing check
        runPairingCheck();

        preparePcsInfo(nCaptx);

        return true;
    }

    // For benchmarking purposes only
    function preparePcsInfo(uint256 nCaptx) internal {
        // $nCaptx$ multi-exp in G1 of size $c$ where c=32
        // (Empirically 29=<c<=36. See rust code call `prepare_pcs_info` in PlonkKzgSnark.batch_verify)

        uint256 c = 32;
        for (uint256 i = 0; i < nCaptx; i++) {
            runMultiExpG1(c);
        }
    }

    // For benchmarking purposes only
    function runPairingCheck() internal {
        Curve.G1Point memory g1 = Curve.P1();
        Curve.G2Point memory g2 = Curve.P2();

        Curve.G1Point[] memory points1 = new Curve.G1Point[](1);
        points1[0] = g1;

        Curve.G2Point[] memory points2 = new Curve.G2Point[](1);
        points2[0] = g2;
        bool res = Curve.pairing(points1, points2); // solhint-disable-line no-unused-vars
    }

    // For benchmarking purposes only
    // TODO use proper multiexp opcode
    function runMultiExpG1(uint256 size) private {
        for (uint256 i = 0; i < size; i++) {
            // Group scalar multiplications
            Curve.G1Point memory g1 = Curve.P1();
            uint256 scalar1 = 545454; // TODO use bigger scalar
            Curve.G1Point memory p1 = Curve.g1mul(g1, scalar1); // solhint-disable-line no-unused-vars

            // (size-1) group additions
            if (i >= 1) {
                Curve.G1Point memory p2 = Curve.g1add(g1, g1); // solhint-disable-line no-unused-vars
            }
        }
    }
}
