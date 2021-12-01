const { expect } = require("chai");
const { ethers } = require("hardhat");
import { hashFrontier } from "../lib/common-tests";
import {
  flattenedFrontier0TreeHeight20,
  flattenedFrontier0TreeHeight3,
  flattenedFrontier1TreeHeight20,
} from "./test-data";
import { TestRecordsMerkleTree } from "../typechain-types";

describe("Records Merkle Tree tests", function () {
  let recordsMerkleTree: TestRecordsMerkleTree;
  let rmtFactory: {
    deploy: (arg0: number) => TestRecordsMerkleTree | PromiseLike<TestRecordsMerkleTree>;
  };

  beforeEach(async function () {
    rmtFactory = await ethers.getContractFactory("TestRecordsMerkleTree");
  });

  it("shows the merkle root is updated correctly and the frontier hash value as well", async function () {
    let TREE_HEIGHT = 20;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let initial_root_value = ethers.BigNumber.from(
      "16338819200219295738128869281163133642735762710891814031809540606861827401155"
    );
    let initial_number_of_leaves = 1;

    let hashFrontierValue = hashFrontier(flattenedFrontier0TreeHeight20, ethers.BigNumber.from(0));

    await recordsMerkleTree.testSetFrontierHashValue(ethers.utils.arrayify(hashFrontierValue));

    let elems = [1, 2, 3, 4, 5];

    await recordsMerkleTree.testSetRootAndNumLeaves(initial_root_value, initial_number_of_leaves);

    const txEmpty = await recordsMerkleTree.testUpdateRecordsMerkleTree(
      flattenedFrontier0TreeHeight20,
      []
    );
    await txEmpty.wait();

    // No elements have been inserted, the stored hash of frontier remains the same.
    let tx = await recordsMerkleTree.testUpdateRecordsMerkleTree(
      flattenedFrontier0TreeHeight20,
      elems
    );
    await tx.wait();

    // Now if we pass an old flattened frontier we get an error
    await expect(
      recordsMerkleTree.testUpdateRecordsMerkleTree(flattenedFrontier0TreeHeight20, [])
    ).to.be.revertedWith("Frontier not consistent w/ state");

    // Passing the new frontier we can insert again
    tx = await recordsMerkleTree.testUpdateRecordsMerkleTree(
      flattenedFrontier1TreeHeight20,
      [17, 38, 33, 66, 77]
    );
    await tx.wait();
  });

  it("inserts all 27 leaves into a merkle tree of height 3", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let initial_root_value = ethers.BigNumber.from(
      "16338819200219295738128869281163133642735762710891814031809540606861827401155"
    );
    let initial_number_of_leaves = 1;

    let hashFrontierValue = hashFrontier(flattenedFrontier0TreeHeight3, ethers.BigNumber.from(0));

    await recordsMerkleTree.testSetFrontierHashValue(ethers.utils.arrayify(hashFrontierValue));

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26,
    ];

    await recordsMerkleTree.testSetRootAndNumLeaves(initial_root_value, initial_number_of_leaves);

    // Passing the new frontier we can insert again
    let tx = await recordsMerkleTree.testUpdateRecordsMerkleTree(
      flattenedFrontier0TreeHeight3,
      elems
    );
    await tx.wait();
  });

  it("shows that inserting too many leaves triggers an error", async function () {
    let TREE_HEIGHT = 3;
    recordsMerkleTree = await rmtFactory.deploy(TREE_HEIGHT);

    let initial_root_value = ethers.BigNumber.from(
      "16338819200219295738128869281163133642735762710891814031809540606861827401155"
    );
    let initial_number_of_leaves = 1;

    let hashFrontierValue = hashFrontier(flattenedFrontier0TreeHeight3, ethers.BigNumber.from(0));

    await recordsMerkleTree.testSetFrontierHashValue(ethers.utils.arrayify(hashFrontierValue));

    let elems = [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26, 27,
    ];

    await recordsMerkleTree.testSetRootAndNumLeaves(initial_root_value, initial_number_of_leaves);

    await expect(
      recordsMerkleTree.testUpdateRecordsMerkleTree(flattenedFrontier0TreeHeight3, elems)
    ).to.be.revertedWith("The tree is full.");
  });
});
