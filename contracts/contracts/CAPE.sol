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
    struct CAPETransaction {
        /// DOC COMMENT IGNORED. Documentation for the field named field.
        bool field; // TODO
    }

    struct AssetDefinition {
        bool field;
        // TODO
    }

    struct RecordOpening {
        bool field;
        // TODO
    }

    struct CapeBlock {
        bool field;
        // TODO
    }

    /// @notice Validate a transaction and if successful apply it.
    /// @dev This is the developer doc for validateAndApply.
    /// @param _block is an array of transactions
    function validateAndApply(CAPETransaction[] calldata _block) internal {}

    /// Insert an asset record.
    /// @param _record is an AssetRecord to insert somewhere
    function insertRecord(AssetRecord memory _record) internal {}

    /// Process a transaction in the standard way (not a burn?)
    /// @param _transaction is a CAPETransaction
    function processStandardTransaction(CAPETransaction memory _transaction)
        internal
    {}

    /// Process a burn transaction.
    /// @param _transaction is a CAPETransaction
    function processBurnTransaction(CAPETransaction memory _transaction)
        internal
    {}

    /// @notice Check if an asset is already registered
    /// @param erc20Address erc20 token address corresponding to the asset type.
    /// @param newAsset asset type.
    /// @return true if the asset type is registered, false otherwise
    function isCapeAssetRegistered(
        address erc20Address,
        AssetDefinition memory newAsset
    ) public returns (bool) {
        return true;
    }

    /// @notice create a new asset type associated to some erc20 token and register it in the contract so that it can be used later for wrapping.
    /// @param _erc20Address erc20 token address of corresponding to the asset type.
    /// @param _newAsset asset type to be registered in the contract.
    function sponsorCapeAsset(
        address _erc20Address,
        AssetDefinition memory _newAsset
    ) public {}

    /// @notice allows to wrap some erc20 tokens into some CAPE asset defined in the record opening
    /// @param _ro record opening that will be inserted in the records merkle tree once the deposit is validated.
    /// @param _erc20Address address of the ERC20 token corresponding to the deposit.
    function depositErc20(RecordOpening memory _ro, address _erc20Address)
        public
    {
        address depositorAddress = msg.sender;
    }

    /// @notice submit a new block to the CAPE contract. Transactions are validated and the blockchain state is updated. Moreover burn transactions trigger the unwrapping of cape asset records into erc20 tokens.
    /// @param _newBlock block to be processed by the CAPE contract.
    /// @param _mtFrontier latest frontier of the records merkle tree.
    /// @param _burnedRos record opening of the second outputs of the burn transactions. The information contained in these records opening allow the contract to transfer the erc20 tokens.
    function submitCapeBlock(
        CapeBlock memory newBlock,
        uint256[] memory mtFrontier,
        RecordOpening[] memory burnedRos
    ) public {}
}
