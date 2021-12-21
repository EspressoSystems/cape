// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../libraries/BN254.sol";

interface IPlonkVerifier {
    // Flatten out TurboPlonk proof
    struct PlonkProof {
        BN254.G1Point wire0; // input wire poly com
        BN254.G1Point wire1;
        BN254.G1Point wire2;
        BN254.G1Point wire3;
        BN254.G1Point wire4; // output wire poly com
        BN254.G1Point prodPerm; // product permutation poly com
        BN254.G1Point split0; // split quotient poly com
        BN254.G1Point split1;
        BN254.G1Point split2;
        BN254.G1Point split3;
        BN254.G1Point split4;
        BN254.G1Point zeta; // witness poly com for aggregated opening at `zeta`
        BN254.G1Point zetaOmega; // witness poly com for shifted opening at `zeta * \omega`
        uint256 wireEval0; // wire poly eval at `zeta`
        uint256 wireEval1;
        uint256 wireEval2;
        uint256 wireEval3;
        uint256 wireEval4;
        uint256 sigmaEval0; // extended permutation (sigma) poly eval at `zeta`
        uint256 sigmaEval1;
        uint256 sigmaEval2;
        uint256 sigmaEval3; // last (sigmaEval4) is saved by Maller Optimization
        uint256 prodPermZetaOmegaEval; // product permutation poly eval at `zeta * \omega`
    }

    /// @dev Verify a single TurboPlonk proof.
    function verify(
        bytes memory verifyingKey,
        uint256[] memory publicInput,
        PlonkProof memory proof,
        bytes memory extraTranscriptInitMsg
    ) external returns (bool);

    // // TODO: To be refined, we might be able to merge some part of verifying keys.
    // /// @dev Batch verify multiple TurboPlonk proofs.
    // function batchVerify(
    //     bytes[] memory verifyingKey,
    //     uint256[][] memory publicInputs,
    //     PlonkProof[] memory proofs,
    //     bytes[] memory extraTranscriptInitMsgs
    // ) external returns (bool);
}
