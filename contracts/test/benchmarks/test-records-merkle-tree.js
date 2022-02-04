const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Records Merkle Tree Benchmarks", function () {
  describe("The Records Merkle root is updated with the frontier.", async function () {
    let owner, rmtContract;

    beforeEach(async function () {
      [owner] = await ethers.getSigners();

      let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
      const RMT = await ethers.getContractFactory("TestRecordsMerkleTree", {
        libraries: {
          RescueLib: rescue.address,
        },
      });
      TREE_HEIGHT = 20;
      rmtContract = await RMT.deploy(TREE_HEIGHT);

      // Polling interval in ms.
      rmtContract.provider.pollingInterval = 20;

      await rmtContract.deployed();
    });

    it("shows how much gas is spent by updateRecordsMerkleTree", async function () {
      let elems = [1, 2, 3, 4, 5];

      const txEmpty = await rmtContract.testUpdateRecordsMerkleTree([]);
      const txEmptyReceipt = await txEmpty.wait();
      let emptyGasUsed = txEmptyReceipt.gasUsed;

      tx = await rmtContract.testUpdateRecordsMerkleTree(elems);
      const txReceipt = await tx.wait();
      let totalGasUsed = txReceipt.gasUsed;

      const doNothingTx = await rmtContract.doNothing();
      const doNothingTxReceipt = await doNothingTx.wait();
      let doNothingGasUsed = doNothingTxReceipt.gasUsed;

      // Total gas used to insert all the records, read from and store into the frontier
      expect(totalGasUsed).to.be.below(3300000);
      console.log("Total gas used to insert all records: ", totalGasUsed);

      // Gas used just to handle the frontier (no records inserted)
      expect(emptyGasUsed).to.be.below(1650000);
      console.log("Gas used just to handle the frontier (no records inserted): ", emptyGasUsed);

      // Gas used to deal with the frontier but without "base" cost
      let handleFrontierGasUsedWithoutBaseCost = emptyGasUsed - doNothingGasUsed;
      expect(handleFrontierGasUsedWithoutBaseCost).to.be.below(150000);
      console.log(
        "Gas used to deal with the frontier but without base cost: ",
        handleFrontierGasUsedWithoutBaseCost
      );

      // Gas used to handle the frontier and insert records but without "base" cost
      let updateRecordsMerkleTreeWithoutBaseCost = totalGasUsed - doNothingGasUsed;
      expect(updateRecordsMerkleTreeWithoutBaseCost).to.be.below(3300000);
      console.log(
        "Gas used to handle the frontier and insert records but without base cost: ",
        updateRecordsMerkleTreeWithoutBaseCost
      );

      // Gas used to insert the records
      let insertRecordsGasUsed = totalGasUsed - emptyGasUsed;
      expect(insertRecordsGasUsed).to.be.below(3300000);
      console.log("Gas used to insert the records: ", insertRecordsGasUsed);
    });
  });
});
