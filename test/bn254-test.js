const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("bn254", function () {
  it("should add two G1 points", async function () {
    const [owner] = await ethers.getSigners();

    const TestBN254 = await ethers.getContractFactory("TestBN254");
    const testBN254 = await TestBN254.deploy();
    await testBN254.deployed();

    const zero = { X: "0", Y: "0" };
    // Returns a merged array/object [0, 0, X: 0, Y: 0]
    ret = await testBN254.callStatic.g1Add(zero, zero);
    expect(ret.X).to.equal(zero.X);
    expect(ret.Y).to.equal(zero.Y);
  });
});
