//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";
import "./NullifiersStore.sol";

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!
contract CAPE is NullifiersStore, Wrapper {
    /// DOC COMMENT IGNORED. Transaction, not sure why it's CAPTransaction and not CapeTransaction
    struct CAPTransaction {
        /// DOC COMMENT IGNORED. Documentation for the field named field.
        bool field; // TODO
    }

    /// @notice Validate a transaction and if successful apply it.
    /// @dev This is the developer doc for validateAndApply.
    /// @param _block is an array of transactions
    function validateAndApply(CAPTransaction[] calldata _block) public {}

    /// @notice Process an ERC-20 deposit
    /// @param _assetType is an AssetType
    /// @param _amount is the number of units
    /// @param _sender is the senders address
    function processERC20Deposits(
        AssetType memory _assetType,
        uint256 _amount,
        address _sender
    ) private {}

    /// Insert an asset record.
    /// @param _record is an AssetRecord to insert somewhere
    function insertRecord(AssetRecord memory _record) internal {}

    /// Process a transaction in the standard way (not a burn?)
    /// @param _transaction is a CAPTransaction
    function processStandardTransaction(CAPTransaction memory _transaction)
        internal
    {}

    /// Process a burn transaction.
    /// @param _transaction is a CAPTransaction
    function processBurnTransaction(CAPTransaction memory _transaction)
        internal
    {}
}
