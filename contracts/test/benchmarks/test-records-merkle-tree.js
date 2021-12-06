const { expect } = require("chai");
const { ethers } = require("hardhat");
const { hashFrontier } = require("../../lib/common-tests");
const { rootValue, flattenedFrontier0TreeHeight20 } = require("../test-data");

describe("Records Merkle Tree Benchmarks", function () {
  describe("The Records Merkle root is updated with the frontier.", async function () {
    let owner, rmtContract;

    beforeEach(async function () {
      [owner] = await ethers.getSigners();

      const RMT = await ethers.getContractFactory("TestRecordsMerkleTree");
      TREE_HEIGHT = 20;
      rmtContract = await RMT.deploy(TREE_HEIGHT);

      // Polling interval in ms.
      rmtContract.provider.pollingInterval = 20;

      await rmtContract.deployed();

      let initial_number_of_leaves = 1;

      await rmtContract.testSetRootAndNumLeaves(rootValue, initial_number_of_leaves);

      let hash_frontier_value = hashFrontier(flattenedFrontier0TreeHeight20, 0);

      let tx = await rmtContract.testSetFrontierHashValue(
        ethers.utils.arrayify(hash_frontier_value)
      );
      await tx.wait();
    });

    it("shows how much gas is spent for checking the frontier", async function () {
      const checkFrontierTx = await rmtContract.testCheckFrontier(flattenedFrontier0TreeHeight20);
      const checkFrontierTxReceipt = await checkFrontierTx.wait();
      let checkFrontierGasUsed = checkFrontierTxReceipt.gasUsed;
      expect(checkFrontierGasUsed).to.be.equal(52682);
    });

    it("shows how much gas is spent to hash the frontier and store this hash", async function () {
      const hashAndStoreTx = await rmtContract.hashFrontierAndStoreHash(
        flattenedFrontier0TreeHeight20,
        1
      );
      const hashAndStoreTxReceipt = await hashAndStoreTx.wait();
      let hashAndStoreGasUsed = hashAndStoreTxReceipt.gasUsed;
      expect(hashAndStoreGasUsed).to.be.equal(53444);
    });

    it("shows how much gas is spent by updateRecordsMerkleTree", async function () {
      let elems = [1, 2, 3, 4, 5];

      const txEmpty = await rmtContract.testUpdateRecordsMerkleTree(
        flattenedFrontier0TreeHeight20,
        []
      );
      const txEmptyReceipt = await txEmpty.wait();
      let emptyGasUsed = txEmptyReceipt.gasUsed;

      tx = await rmtContract.testUpdateRecordsMerkleTree(flattenedFrontier0TreeHeight20, elems);
      const txReceipt = await tx.wait();
      let totalGasUsed = txReceipt.gasUsed;

      const doNothingTx = await rmtContract.doNothing();
      const doNothingTxReceipt = await doNothingTx.wait();
      let doNothingGasUsed = doNothingTxReceipt.gasUsed;

      // Total gas used to check the frontier and insert all the records
      expect(totalGasUsed).to.be.equal(2704300);

      // Gas used just to check the frontier (no records inserted)
      expect(emptyGasUsed).to.be.equal(63167);

      // Gas used to check the frontier but without "base" cost
      let checkFrontierGasUsedWithoutBaseCost = emptyGasUsed - doNothingGasUsed;
      expect(checkFrontierGasUsedWithoutBaseCost).to.be.equal(41982);

      // Gas used to check the frontier and insert records but without "base" cost
      let updateRecordsMerkleTreeWithoutBaseCost = totalGasUsed - doNothingGasUsed;
      expect(updateRecordsMerkleTreeWithoutBaseCost).to.be.equal(2683115);

      // Gas used to insert the records
      let insertRecordsGasUsed = totalGasUsed - emptyGasUsed;
      expect(insertRecordsGasUsed).to.be.equal(2641133);
    });
  });
});
