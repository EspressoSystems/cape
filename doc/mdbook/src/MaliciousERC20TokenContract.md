In this section we analyze a possible DoS attack on a relayer made possible by a malicious ERC20 Token.
The attack works as follows:

* Deploy an ERC20 CrashCoin with well-behaved `allow()` and `transferFrom`, but `transfer` reverts immediately.
* Wrap 1 `CrashCoin` in CAPE.
* Submit a `CrashCoin` burn/unwrap transaction to a relayer.
* The relayer includes it in the block.
* The block gets "rejected" when it calls `CrashCoin.transfer`.

Possible mitigations:
1) The relayer could try to run the Ethereum transaction first. This would probably catch most of these cases. The user could however use a token that calls to a proxy and frontrun the relayer's TX to change the token to become malicious before the real TX goes through.
2) Only whitelisted tokens can be sponsored. 
3) Instead of withdrawing during the block submission we just do the bookkeeping and mark funds as "available for withdrawal to address". The user later needs to run the withdraw transaction that moves the funds. 