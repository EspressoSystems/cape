const { expect } = require("chai");
const { ethers } = require("hardhat");

const MODEXP_3 = "4407920970296243842837207485651524041948558517760411303933";
const exponent =
  "0xc19139cb84c680a6e14116da060561765e05aa45a1c72a34f082305b61f3f52";
const modulus =
  "0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47";

describe("ExpMod", function () {
  it("Should compute exp mod", async function () {
    const [owner] = await ethers.getSigners();

    const Contract = await ethers.getContractFactory("ExpMod");
    const contract = await Contract.deploy();
    await contract.deployed();

    const actual = await contract.callStatic.expmod("3", exponent, modulus);
    expect(actual).to.equal(MODEXP_3);
  });
});
