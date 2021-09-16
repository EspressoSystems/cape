const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Dummy Plonk Verifier", function () {

  it("Should return the compute the gas fee", async function () {

    const [owner] = await ethers.getSigners();

    const DPV = await ethers.getContractFactory("DummyPlonkVerifier");
    const dpv = await DPV.deploy();
    await dpv.deployed();

    // TODO increment the AAP block size
    let aap_bytes_size = 3000;
    let n_app_tx = 20;
    let bytes_len = n_app_tx * aap_bytes_size;

    let chunk = new Uint8Array(bytes_len);
    for (let i=0; i< bytes_len; i++){
      chunk[i] = 12;
    }

    let tx = await dpv.verify(chunk);
    let txReceipt = await tx.wait();

    // TODO why does this not work
    // expect(tx).to.equal(true);

    let gasUsed = txReceipt.cumulativeGasUsed.toString();
    let expectedGasUsed = ethers.BigNumber.from("5117998");
    expect(gasUsed).equal(expectedGasUsed);
  });
});
