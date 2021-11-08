//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";
import "./NullifiersStore.sol";

contract AAPE is NullifiersStore, Wrapper {
    struct AAPTransaction {
        bool field; // TODO
    }

    function validateAndApply(AAPTransaction[] calldata _block) public {}

    function processERC20Deposits(
        AssetType memory _assetType,
        uint256 _amount,
        address _sender
    ) private {}

    function insertRecord(AssetRecord memory _record) internal {}

    function processStandardTransaction(AAPTransaction memory _transaction)
        internal
    {}

    function processBurnTransaction(AAPTransaction memory _transaction)
        internal
    {}
}
