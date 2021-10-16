const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Nullifiers Merkle tree", function () {
  it("should compute correctly the hash functions", async function () {
    const [owner] = await ethers.getSigners();

    const NullifiersMerkleTree = await ethers.getContractFactory(
      "NullifiersMerkleTree"
    );
    const nf_merkle_tree = await NullifiersMerkleTree.deploy();
    await nf_merkle_tree.deployed();
    let _res = await nf_merkle_tree.callStatic.elem_hash(10000);
  });
});
