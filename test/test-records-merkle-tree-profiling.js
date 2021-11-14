const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Records Merkle Tree Profiling", function () {
  describe("The Records Merkle root is updated with the frontier.", async function () {
    let owner, rmt_contract;

    beforeEach(async function () {
      [owner] = await ethers.getSigners();

      const RMT = await ethers.getContractFactory("TestRecordsMerkleTree");
      TREE_HEIGHT = 20;
      rmt_contract = await RMT.deploy(TREE_HEIGHT);

      // Polling interval in ms.
      rmt_contract.provider.pollingInterval = 20;

      await rmt_contract.deployed();
    });

    it("works", async function () {
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

      await rmt_contract.testSetRootAndNumLeaves(
        initial_root_value,
        initial_number_of_leaves
      );

      const tx_empty = await rmt_contract.testUpdateRecordsMerkleTree(
        flattened_frontier,
        []
      );
      const tx_emptyReceipt = await tx_empty.wait();
      let empty_gasUsed = tx_emptyReceipt.gasUsed;

      const tx = await rmt_contract.testUpdateRecordsMerkleTree(
        flattened_frontier,
        elems
      );
      const txReceipt = await tx.wait();
      let gasUsed = txReceipt.gasUsed;

      const rescue_tx = await rmt_contract.hash(10, 15, 20);
      const rescue_txReceipt = await rescue_tx.wait();
      let rescue_gasUsed = rescue_txReceipt.gasUsed;

      const do_nothing_tx = await rmt_contract.doNothing();
      const do_nothing_txReceipt = await do_nothing_tx.wait();
      let do_nothing_gasUsed = do_nothing_txReceipt.gasUsed;

      console.log("Tree height:" + TREE_HEIGHT.toString());
      console.log("nothing: " + do_nothing_gasUsed.toString());
      console.log("rescue: " + rescue_gasUsed.toString());
      console.log(
        "rescue only: " + (rescue_gasUsed - do_nothing_gasUsed).toString()
      );
      console.log(
        "testUpdateRecordsMerkleTree empty: " + empty_gasUsed.toString()
      );
      console.log(
        "testUpdateRecordsMerkleTree empty only: " +
          (empty_gasUsed - do_nothing_gasUsed).toString()
      );
      console.log("Number of records: " + elems.length.toString());
      console.log("testUpdateRecordsMerkleTree: " + gasUsed.toString());
      console.log(
        "testUpdateRecordsMerkleTree only: " +
          (gasUsed - do_nothing_gasUsed).toString()
      );
      console.log("insertion gas: " + (gasUsed - empty_gasUsed).toString());
      expect(gasUsed).lt(20_000_000);
    });
  });
});
