//SPDX-License-Identifier: Unlicense
pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import { Curve } from "./BN254.sol";

contract DummyPlonkVerifier {

    constructor() public {
    }

    function verify_empty(bytes memory chunk) public returns (bool) {
        return true;
    }

    function verify(bytes memory chunk) public returns (bool) {
        // Count the number of transactions
        uint aaptx_size =  3000;
        uint n_aaptx =  chunk.length / aaptx_size;

        // n_aaptx pairing check
        for (uint i=0;i<n_aaptx;i++){
            run_pairing_check();
        }

        // Cost of prepare_pcs_info
        prepare_pcs_info(n_aaptx);

        return true;
    }

    function prepare_pcs_info(uint n_aaptx) private {
        // $n_aaptx$ multi-exp in G1 of size $c$ where c=32
        // (Empirically 29=<c<=36. See rust code call `prepare_pcs_info` in PlonkKzgSnark.batch_verify)

        uint c = 32;
        for (uint i=0;i<n_aaptx;i++){
            run_multi_exp_g1(c);
        }
    }

    function batch_verify(bytes memory chunk) public returns (bool) {
        // Count the number of transactions
        uint aaptx_size =  3000;
        uint n_aaptx =  chunk.length / aaptx_size;

        // We lower bound the complexity by
        // 1 pairing check
        // 2  multi exp in G1 of size $n_aaptx$ (See rust code PlonkKzgSnark.batch_verify)
        // Cost of prepare_pcs_info(n_aaptx)

        // 2 multi exp in G1 of size $n_aaptx$
        run_multi_exp_g1(n_aaptx);
        run_multi_exp_g1(n_aaptx);

        // 1 pairing check
        run_pairing_check();

        prepare_pcs_info(n_aaptx);

        return true;
    }

    function run_pairing_check() private {
        Curve.G1Point memory g1 = Curve.P1();
        Curve.G2Point memory g2 = Curve.P2();

        Curve.G1Point [] memory points1 = new Curve.G1Point[](1);
        points1[0] = g1;

        Curve.G2Point [] memory points2 = new Curve.G2Point[](1);
        points2[0] = g2;
        bool res = Curve.pairing(points1, points2);
    }

    // TODO use proper multiexp opcode
    function run_multi_exp_g1(uint size) private {
        for (uint i=0;i<size;i++) {
            // Group scalar multiplications
            Curve.G1Point memory g1 = Curve.P1();
            uint scalar1 = 545454; // TODO use bigger scalar
            Curve.G1Point memory p1 = Curve.g1mul(g1,scalar1);

            // (size-1) group additions
            if (i>=1) {
                Curve.G1Point memory p2 = Curve.g1add(g1,g1);
            }
        }
    }
}
