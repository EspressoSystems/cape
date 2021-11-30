//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./NullifiersStore.sol";
import "./RecordsMerkleTree.sol";
import "./PlonkVerifier.sol";

contract DummyCAPE is NullifiersStore, RecordsMerkleTree, PlonkVerifier {
    uint8 public constant RECORDS_TREE_HEIGHT = 25;
    uint256 public constant CAPTX_SIZE = 3000; // Must be the same as in the javascript testing code
    uint256 public constant N_INPUTS = 4; // Number of AAP inputs per transactions corresponding to a transaction of roughly 3 KB
    uint256 public constant N_OUTPUTS = 5; // Number of AAP outputs per transactions of roughly 3 KB

    /* solhint-enable */

    constructor() RecordsMerkleTree(RECORDS_TREE_HEIGHT) {}

    function verifyEmpty(
        bytes memory chunk // solhint-disable-line no-unused-vars
    ) public returns (bool) {
        return true;
    }

    function verify(bytes memory chunk) public returns (bool) {
        // Count the number of transactions
        uint256 nCaptx = chunk.length / CAPTX_SIZE;

        // Cost of plonk verification
        batchVerify(chunk);

        // Cost of updating the Merkle tree
        updateMerkleTrees(nCaptx);

        return true;
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
}
