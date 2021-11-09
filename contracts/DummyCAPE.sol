//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import {Curve} from "./BN254.sol";
import "./NullifiersStore.sol";
import "./RecordsMerkleTree.sol";

contract DummyCAPE is NullifiersStore, RecordsMerkleTree {
    uint64 public constant RECORDS_TREE_HEIGHT = 25;
    uint256 public constant CAPTX_SIZE = 3000; // Must be the same as in the javascript testing code
    uint256 public constant N_INPUTS = 4; // Number of AAP inputs per transactions corresponding to a transaction of roughly 3 KB
    uint256 public constant N_OUTPUTS = 5; // Number of AAP outputs per transactions of roughly 3 KB

    /* solhint-enable */

    constructor() public RecordsMerkleTree(RECORDS_TREE_HEIGHT) {}

    function verifyEmpty(
        bytes memory chunk, // solhint-disable-line no-unused-vars
        bool merkleTreesUpdate // solhint-disable-line no-unused-vars
    ) public returns (bool) {
        return true;
    }

    function verify(bytes memory chunk, bool merkleTreesUpdate)
        public
        returns (bool)
    {
        // Count the number of transactions
        uint256 nCaptx = chunk.length / CAPTX_SIZE;

        // nCaptx pairing check
        for (uint256 i = 0; i < nCaptx; i++) {
            runPairingCheck();
        }

        // Cost of prepare_pcs_info
        preparePcsInfo(nCaptx);

        if (merkleTreesUpdate) {
            updateMerkleTrees(nCaptx);
        }

        return true;
    }

    function preparePcsInfo(uint256 nCaptx) private {
        // $nCaptx$ multi-exp in G1 of size $c$ where c=32
        // (Empirically 29=<c<=36. See rust code call `prepare_pcs_info` in PlonkKzgSnark.batch_verify)

        uint256 c = 32;
        for (uint256 i = 0; i < nCaptx; i++) {
            runMultiExpG1(c);
        }
    }

    function insertNullifiers() private returns (bytes32) {
        bytes memory nullifier = "a857857";
        for (uint256 i = 0; i < N_INPUTS; i++) {
            insertNullifier(nullifier);
        }
    }

    /*
        Simulation of the batch insertion of elements in the ternary Merkle tree
        based on Joe's observation:
        Instead of inserting elements one by one which would cost H*N where N is the number of
        elements and H is the height of the tree, the idea is to insert the elements in batch,
        saving by doing so many calls to the rescue hash function.
        The cost is N + N/3 + N/9 + ... + N/(3^H) which tends to 1.5 N when N grows.
        So we approximate the cost of inserting N elements  in the tree by the cost of computing 1.5 N rescue hash functions.
     */
    function updateRecordsTreeBatch(uint256 nCaptx) private {
        uint256 totalCostBatchInsertion = (3 * nCaptx * N_OUTPUTS) / 2;

        for (uint256 i = 0; i < totalCostBatchInsertion; i++) {
            // Computes rescue hash
            uint256 a = 7878754242;
            uint256 b = 468777777777776575;
            uint256 c = 87875474574;
            Rescue.hash(a, b, c);
        }
    }

    function updateMerkleTrees(uint256 nCaptx) public {
        // For the nullifier tree we insert the leaves one by one
        for (uint256 i = 0; i < nCaptx; i++) {
            insertNullifiers();
        }

        // For the record tree we insert the records in batch
        updateRecordsTreeBatch(nCaptx);
    }

    function batchVerify(bytes memory chunk, bool merkleTreesUpdate)
        public
        returns (bool)
    {
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

        if (merkleTreesUpdate) {
            updateMerkleTrees(nCaptx);
        }

        return true;
    }

    function runPairingCheck() private {
        Curve.G1Point memory g1 = Curve.P1();
        Curve.G2Point memory g2 = Curve.P2();

        Curve.G1Point[] memory points1 = new Curve.G1Point[](1);
        points1[0] = g1;

        Curve.G2Point[] memory points2 = new Curve.G2Point[](1);
        points2[0] = g2;
        bool res = Curve.pairing(points1, points2); // solhint-disable-line no-unused-vars
    }

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
