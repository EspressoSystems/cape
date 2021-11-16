import { TestBN254 } from "../typechain-types";
import { expect } from "chai";
import { ethers } from "hardhat";
import { G1PointStruct } from "../typechain-types/TestBN254";

describe("bn254", function () {
  let bn254: TestBN254;

  beforeEach(async () => {
    const factory = await ethers.getContractFactory("TestBN254");
    bn254 = await factory.deploy();
  });
  it("should add two G1 points", async function () {
    const zero: G1PointStruct = { X: "0", Y: "0" };
    let res = await bn254.callStatic.g1Add(zero, zero);
    // Returns a merged array/object [0, 0, X: 0, Y: 0]
    expect(res.X).to.equal("0");
    expect(res.Y).to.equal("0");
  });
});
