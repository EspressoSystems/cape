//SPDX-License-Identifier: Unlicense
pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import { Curve } from "./BN254.sol";

contract DummyPlonkVerifier {

    constructor() public {
    }

    function verify(bytes memory chunk) public returns (bool) {
        // Count the number of transactions
        uint aaptx_size =  3000;
        uint n_aaptx =  chunk.length / aaptx_size;

        // Run the plonk verifier once for each AAP transaction
        for (uint i=0; i<n_aaptx; i++) {
            verify_plonk_proof();
        }
        return true;
    }

    function batch_verify(bytes memory chunk) public returns (bool) {
        // Count the number of transactions
        uint aaptx_size =  3000;
        uint n_aaptx =  chunk.length / aaptx_size;

        // We lower bound the complexity by
        // 2 pairings operations
        // 2 $n_aaptx$ multi exp in G1

        run_multi_exp_g1(n_aaptx);

        run_pairing_check();
        run_pairing_check();

        return true;
    }

    function verify_plonk_proof() public returns (bool){
        // From "PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge"
        // https://eprint.iacr.org/2019/953/20210719:164544, page 30, "Verifier algorithm"

        // TODO
        // Step1
        // Validate ([a]_1,[b]_1,[c]_1,[z]_1,[t_{l0}]_1, [t_mid]_1,[t_hi]_1,[W_z],[W_{z\omega}]_1) \in G_1

        // TODO
        // Step 2
        // Validate (\bar{a},\bar{b},\bar{c},\bar{s_{\sigma1}},\bar{s_{\sigma2}}, \bar{s_{z_\omega}}} \in F_p

        // TODO
        // Step 3
        // Validate (w_i)_{i \in l} \in F_p

        // TODO
        // Step 4
        // Compute challenges \beta, \gamma, \alpha, z, v, u \in F_p as in prover's description from the common inputs
        // public input, and elements of \pi_{snark}

        // TODO
        // Step 5
        // Compute zero polynomial evaluation Z_H(z)=z^n -1

        // TODO
        // Step 6
        // Compute Lagrange polynomial evaluation L_1(z)= \frac{\omega(z^n-1)}{n(z-\omega)}

        // TODO
        // Step 7
        // Compute public input polynomial evaluation
        // PI(z) = \sum_{i \in l}L_i(z)

        // TODO
        // Step 8
        // Compute r's constant term
        // r_0 = PI(z)-L1(z)\alpha^2 - \alpha(\bar{a} + \beta \bar{s_{\sigma1}} + \gamma)(\bar{b} + \beta \bar{\sigma_2} + \gamma)(\bar{c} +\gamma)\bar{z_{\omega}}
        // let r'(X)= r(X)-r_0

        // TODO
        // Step 9
        // Compute the first part of batched polynomial commitment
        // [D]_1 = [r']_1 + u[z]_1
        // Cost: 16 F_p multiplications
        //       12 F_p sums
        //       G_1 multi exponentiations of size 11
        run_multi_exp_g1(11);

        // TODO
        // Step 10
        // Compute full batched polynomial commitment [F]_1
        // Cost: 4 F_p multiplications
        //       G_1 multi exponentiations of size 6
        run_multi_exp_g1(6);

        // TODO
        // Step 11
        // Group encoded batch evaluation [E]_1
        // Cost: 6 F_p sums
        //       10 F_p multiplications


        // Step 12
        // Batch validate all evaluations
        // Cost 1 pairing check
        run_pairing_check();

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
