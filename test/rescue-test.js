const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Rescue", function () {
  let contract;

  beforeEach(async function () {
    const Contract = await ethers.getContractFactory("Rescue");
    contract = await Contract.deploy();
    await contract.deployed();
  });

  it("can call .hash", async function () {
    const tx = await contract.hash(1, 2, 3);
    const receipt = await tx.wait();
    // expect(receipt).to.equal(0);
    // expect().to.equal(
    //   "9084330654817845835997205722524578895477688063555937388415796748573681505264"
    // );
  });

  it("check gas", async function () {
    const tx = await contract.myGas();
    const receipt = await tx.wait();
    expect(receipt.events[0].args[0]).to.deep.equal(0);
  });
});
