const { expect } = require("chai");
const { ethers } = require("hardhat");
const common = require("../lib/common");

describe("Nullifiers Merkle tree", function () {
  it("should compute correctly the hash functions", async function () {
    let nf_merkle_tree = await common.deployNullifierMerkleTreeContract();
    let _res = await nf_merkle_tree.callStatic.elem_hash(10000);
  });

  it("should compute the terminal node value", async function () {
    const contract = await common.deployNullifierMerkleTreeContract();

    // fails at
    //    height=262 against geth
    //    heigth=? against arbitrum dev node
    // but it's not entirely deterministic

    for (let height = 262; height < 263; height++) {
      //console.error("height", height);
      let tx = await contract.terminalNodeValueNonEmpty({
        isEmptySubtree: false,
        height: height,
        elem: ethers.utils.randomBytes(32),
      });
      await tx.wait();
    }
  });
});
