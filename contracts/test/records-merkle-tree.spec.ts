const { expect } = require("chai");
const { ethers } = require("hardhat");

import { TestRecordsMerkleTree } from "../typechain-types";

describe("Records Merkle Tree tests", function () {
  let recordsMerkleTree: TestRecordsMerkleTree;
  let rmtFactory: {
    deploy: (arg0: number) => TestRecordsMerkleTree | PromiseLike<TestRecordsMerkleTree>;
  };

  beforeEach(async function () {
    let rescue = await (await ethers.getContractFactory("RescueLib")).deploy();
    rmtFactory = await ethers.getContractFactory("TestRecordsMerkleTree", {
      libraries: {
        RescueLib: rescue.address,
      },
    });
  });

  it("inserts all 27 leaves into a merkle tree of height 3", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26,
    ];

    // Insert all these elements does not trigger an error
    let tx = await recordsMerkleTree.testUpdateRecordsMerkleTree(elems);
    await tx.wait();
  });

  it("shows that inserting too many leaves triggers an error", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26, 27, 28,
    ];

    await expect(recordsMerkleTree.testUpdateRecordsMerkleTree(elems)).to.be.revertedWith(
      "The tree is full."
    );
  });
});
