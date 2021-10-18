const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common");

describe("Nullifiers Merkle tree", function () {
  it("should compute correctly the hash functions", async function () {
    let nf_merkle_tree = await deployNullifierMerkleTreeContract();
    let _res = await nf_merkle_tree.callStatic.elem_hash(10000);
  });

  it("should compute the terminal node value", async function () {
    const contract = await common.deployNullifierMerkleTreeContract();

    // fails at
    //    height=240 against geth
    //    heigth=131 against arbitrum dev node
    // but it's not entirely deterministic

    for (let height = 230; height < 513; height++) {
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
