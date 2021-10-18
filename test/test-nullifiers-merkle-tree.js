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

  it.skip("should compute the terminal node value", async function () {
    const [owner] = await ethers.getSigners();

    const Contract = await ethers.getContractFactory("NullifiersMerkleTree");
    const contract = await Contract.deploy();
    await contract.deployed();
    // fails at
    //    height=25 against geth
    //    heigth=32 against arbitrum dev node
    for (let height = 20; height < 512; height++) {
      console.error("height", height);
      let res = await contract.callStatic.terminalNodeValueNonEmpty(
        {
          isEmptySubtree: false,
          height: height,
          elem: [0, 0, 0, 0, 0, 0, 0, 0],
        }
        // { gasLimit: 25_000_000 }
      );
      // console.error(res);
    }
  });
});
