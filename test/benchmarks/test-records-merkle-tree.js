const { expect } = require("chai");
const { ethers } = require("hardhat");

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
    });

    it("shows how much gas is spent by updateRecordsMerkleTree", async function () {
      let initial_root_value = ethers.BigNumber.from(
        "16338819200219295738128869281163133642735762710891814031809540606861827401155"
      );
      let initial_number_of_leaves = 1;
      let leaf_value = ethers.BigNumber.from(
        "17101599813294219906421080963940202236242422543188383858545041456174912634953"
      );
      let flattened_frontier = [
        leaf_value,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
      ];

      let elems = [1, 2, 3, 4, 5];

      await rmtContract.testSetRootAndNumLeaves(initial_root_value, initial_number_of_leaves);

      const txEmpty = await rmtContract.testUpdateRecordsMerkleTree(flattened_frontier, []);
      const txEmptyReceipt = await txEmpty.wait();
      let emptyGasUsed = txEmptyReceipt.gasUsed;

      const tx = await rmtContract.testUpdateRecordsMerkleTree(flattened_frontier, elems);
      const txReceipt = await tx.wait();
      let totalGasUsed = txReceipt.gasUsed;

      const doNothingTx = await rmtContract.doNothing();
      const doNothingTxReceipt = await doNothingTx.wait();
      let doNothingGasUsed = doNothingTxReceipt.gasUsed;

      // Total gas used to check the frontier and insert all the records
      expect(totalGasUsed).to.be.equal(4410470);

      // Gas used just to check the frontier (no records inserted)
      expect(emptyGasUsed).to.be.equal(1871455);

      // Gas used to check the frontier but without "base" cost
      let checkFrontierGasUsedWithoutBaseCost = emptyGasUsed - doNothingGasUsed;
      expect(checkFrontierGasUsedWithoutBaseCost).to.be.equal(1850270);

      // Gas used to check the frontier and insert records but without "base" cost
      let updateRecordsMerkleTreeWithoutBaseCost = totalGasUsed - doNothingGasUsed;
      expect(updateRecordsMerkleTreeWithoutBaseCost).to.be.equal(4389285);

      // Gas used to insert the records
      let insertRecordsGasUsed = totalGasUsed - emptyGasUsed;
      expect(insertRecordsGasUsed).to.be.equal(2539015);
    });
  });
});
