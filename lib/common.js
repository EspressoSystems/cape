const bigDecimal = require("js-big-decimal");
var rand = require("random-seed").create();

const ETH_PRICE_USD = 3962;

module.exports = {
  create_chunk: function (n_cap_tx) {
    const cap_bytes_size = 3000;

    const bytes_len = n_cap_tx * cap_bytes_size;

    let chunk = new Uint8Array(bytes_len);

    // Generate random looking chunk as the CAP transaction contain random values
    rand.initState();
    for (let i = 0; i < bytes_len; i++) {
      let v = rand(2 ** 8 - 1);
      chunk[i] = v;
    }

    return chunk;
  },

  compute_gas_and_price: async function (owner, fun_to_evaluate, args) {
    // Balance before
    let balance_before = await owner.getBalance();

    const tx = await fun_to_evaluate(...args);
    const txReceipt = await tx.wait();
    const gasUsed = txReceipt.gasUsed.toString();

    // Balance after
    let balance_after = await owner.getBalance();

    let wei_cost = BigInt(balance_before) - BigInt(balance_after);

    const WEI_PER_ETH = 10 ** 18;
    const wei_cost_big_decimal = new bigDecimal(wei_cost.toString());
    const eth_price_usd_big_decimal = new bigDecimal(ETH_PRICE_USD.toString());
    const wei_per_eth_big_decimal = new bigDecimal(WEI_PER_ETH.toString());
    const price = wei_cost_big_decimal
      .multiply(eth_price_usd_big_decimal)
      .divide(wei_per_eth_big_decimal);
    return [gasUsed, price];
  },
};
