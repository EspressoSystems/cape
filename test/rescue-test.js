const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Rescue", function () {
  it("runs", async function () {
    const [owner] = await ethers.getSigners();

    const Contract = await ethers.getContractFactory("Rescue");
    const contract = await Contract.deploy();
    await contract.deployed();

    const tx = await contract.hash(1, 2, 3);
    const receipt = await tx.wait();

    // expect().to.equal(
    //   "9084330654817845835997205722524578895477688063555937388415796748573681505264"
    // );
  });
});
