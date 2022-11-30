// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import { expect } from "chai";
import { ethers } from "hardhat";
import { BigNumber, BigNumberish } from "ethers";
const { utils } = ethers;

describe("Token", function () {
  let contract: any;

  beforeEach(async () => {
    const factory = await ethers.getContractFactory("USDC");
    contract = await factory.deploy();
  });

  it("Mints and withdraws correctly", async function () {
    const [owner] = await ethers.getSigners();
    const provider = new ethers.providers.JsonRpcProvider();
    const decimals = await contract.decimals();
    let receipts = [];

    let beforeEther = await provider.getBalance(owner.address);
    let beforeToken = await contract.balanceOf(owner.address);

    // Wrap some Ether.
    const etherAmount = BigNumber.from("123");
    const tokenAmount = utils.parseUnits(etherAmount.toString(), decimals);
    let tx = await owner.sendTransaction({
      to: contract.address,
      value: utils.parseEther(etherAmount.toString()),
    });
    await tx.wait();
    receipts.push(await provider.getTransactionReceipt(tx.hash));

    let afterToken = await contract.balanceOf(owner.address);

    expect(afterToken).to.equal(beforeToken.add(tokenAmount));

    // Unwrap the tokens back into Ether.
    tx = await contract.withdraw({ gasLimit: 1000000 }); // unpredictable gas limit due to storage free
    await tx.wait();
    receipts.push(await provider.getTransactionReceipt(tx.hash));

    const gasFee = receipts.reduce((acc, receipt) => {
      return acc.add(receipt.gasUsed.mul(receipt.effectiveGasPrice));
    }, BigNumber.from(0));

    // Token balance is zero.
    expect(await contract.balanceOf(owner.address)).to.equal(0);

    // Contract has zero Ether.
    expect(await contract.balanceOf(contract.address)).to.equal(0);

    // Note: This test can get flaky if geth has been running for a long time
    // and the last check may fail in that case. It's therefore currently disabled.

    // Ether balance is the same as originally (minus gas fees)
    // expect(await provider.getBalance(owner.address)).to.equal(beforeEther.sub(gasFee));
  });
});
