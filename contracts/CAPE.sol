//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";
import "./NullifiersStore.sol";

/// CAPE provides auditable anonymous payments on Ethereum.
/// @dev Developers are awesome!
contract CAPE is NullifiersStore, Wrapper {
    /// DOC COMMENT IGNORED. Transaction, not sure why it's CAPTransaction and not CapeTransaction
    struct CAPTransaction {
        /// DOC COMMENT IGNORED. Documentation for the field named field.
        bool field; // TODO
    }

    /// Validate a transaction and if successful apply it.
    /// @dev This is the developer doc for validateAndApply.
    function validateAndApply(CAPTransaction[] calldata _block) public {}

    /// Do private functions appear?
    function processERC20Deposits(
        AssetType memory _assetType,
        uint256 _amount,
        address _sender
    ) private {}

    /// Insert an asset record.
    function insertRecord(AssetRecord memory _record) internal {}

    /// Process a transaction.
    function processStandardTransaction(CAPTransaction memory _transaction)
        internal
    {}

    /// Process a burn transaction.
    function processBurnTransaction(CAPTransaction memory _transaction)
        internal
    {}
}
