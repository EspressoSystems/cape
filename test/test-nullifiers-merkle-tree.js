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

  it("should compute the terminal node value", async function () {
    const [owner] = await ethers.getSigners();

    const Contract = await ethers.getContractFactory("NullifiersMerkleTree");
    const contract = await Contract.deploy();
    await contract.deployed();
    // fails at
    //    height=131 against geth
    //    heigth=131 against arbitrum dev node
    // but it's not entirely deterministic
    for (let height = 130; height < 512; height++) {
      console.error("height", height);
      let tx = await contract.terminalNodeValueNonEmpty({
        isEmptySubtree: false,
        height: height,
        elem: ethers.utils.randomBytes(32),
      });
      await tx.wait();
    }
  });
});
