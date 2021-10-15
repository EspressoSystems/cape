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
    let res = await nf_merkle_tree.callStatic.elem_hash(10000);

    let left = [
      164, 230, 176, 83, 63, 76, 26, 84, 80, 74, 26, 218, 108, 85, 15, 89, 143,
      190, 230, 64, 41, 8, 95, 147, 172, 148, 65, 65, 212, 83, 116, 209, 190,
      83, 69, 169, 34, 229, 112, 248, 7, 52, 90, 207, 224, 84, 171, 3, 34, 76,
      189, 250, 80, 80, 242, 43, 249, 89, 252, 120, 133, 5, 159, 152,
    ];
    let right = [
      112, 245, 251, 246, 253, 242, 101, 88, 40, 165, 103, 61, 155, 197, 150,
      18, 167, 236, 24, 228, 67, 193, 132, 86, 153, 205, 130, 77, 94, 89, 21,
      35, 195, 134, 169, 252, 108, 188, 221, 249, 57, 24, 221, 38, 164, 176,
      208, 63, 10, 35, 45, 63, 65, 185, 32, 252, 48, 200, 33, 191, 104, 220, 90,
      129,
    ];

    res = await nf_merkle_tree.callStatic.branch_hash(left, right);
    console.log(res);
  });
});
