const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("bn254", function () {

  it("should add two G1 points", async function () {

    const [owner] = await ethers.getSigners();

    const TestBN254 = await ethers.getContractFactory("testBN254");
    const testBN254 = await TestBN254.deploy();
    await testBN254.deployed();

    expect(await testBN254.g1Add()).to.equal(true);

  });
  
});
