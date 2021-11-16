import { expect } from "chai";
import { ethers } from "hardhat";
import { Greeter } from "@typechain-types";

describe("Greeter (typescript)", function () {
  it("Should return the new greeting once it's changed", async function () {
    const Greeter = await ethers.getContractFactory("Greeter");
    const greeter = await Greeter.deploy("Hello, world!");
    await greeter.deployed();

    expect(await greeter.greet()).to.equal("Hello, world!");

    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");

    // wait until the transaction is mined
    await setGreetingTx.wait();

    let greeting_str = await greeter.greet();
    expect(greeting_str).to.equal("Hola, mundo!");
  });
});
