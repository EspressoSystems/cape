import { expect } from "chai";
import { ethers } from "hardhat";
import { Rescue, RescueNonOptimized } from "../typechain-types";

describe("Rescue", function () {
  // const contracts = ["Rescue", "RescueNonOptimized"];
  // const contracts = ["RescueNonOptimized"];

  // contracts.forEach((contractName) => {
  // let contract: Rescue | RescueNonOptimized;
  let contract: Rescue;

  beforeEach(async function () {
    // const Contract = await ethers.getContractFactory(contractName);
    const Contract = await ethers.getContractFactory("Rescue");
    contract = await Contract.deploy();
    await contract.deployed();
  });

  // it("can call .expMod", async function () {
  //   const a = 7878754242;
  //   const b = ethers.BigNumber.from("468777777777776575");
  //   const c = 87875474574;
  //   const tx = await contract.expMod(a, b, c);
  //   const receipt = await tx.wait();
  //   // expect(receipt).to.equal(0);
  //   // expect().to.equal(
  //   //   "9084330654817845835997205722524578895477688063555937388415796748573681505264"
  //   // );
  // });

  // it("can call .expModMulMod", async function () {
  //   const tx = await contract.expModMulMod(1, 2, 3);
  //   const receipt = await tx.wait();
  //   // expect(receipt).to.equal(0);
  //   // expect().to.equal(
  //   //   "9084330654817845835997205722524578895477688063555937388415796748573681505264"
  //   // );
  // });

  it("can call .hash", async function () {
    const tx = await contract.hash(
      1,
      2,
      3
      // { gasLimit: 20_000_000 }
    );
    const receipt = await tx.wait();
    // expect(receipt).to.equal(0);
    // expect().to.equal(
    //   "9084330654817845835997205722524578895477688063555937388415796748573681505264"
    // );
  });

  it("check gas", async function () {
    const tx = await contract
      .myGas
      // { gasLimit: 20_000_000 }
      ();
    const receipt = await tx.wait();
    console.error(JSON.stringify(receipt, null, 2));
    expect(receipt.events![0].args![0]).to.deep.equal(0);
  });
  // });
});
