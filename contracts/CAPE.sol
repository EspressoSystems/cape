//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";
import "./NullifiersStore.sol";

contract CAPE is NullifiersStore, Wrapper {
    struct CAPTransaction {
        bool field; // TODO
    }

    function validateAndApply(CAPTransaction[] calldata _block) public {}

    function processERC20Deposits(
        AssetType memory _assetType,
        uint256 _amount,
        address _sender
    ) private {}

    function insertRecord(AssetRecord memory _record) internal {}

    function processStandardTransaction(CAPTransaction memory _transaction)
        internal
    {}

    function processBurnTransaction(CAPTransaction memory _transaction)
        internal
    {}
}
