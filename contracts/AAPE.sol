//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";
import "./NullifiersStore.sol";

contract AAPE is NullifiersStore, Wrapper {
    struct AAPTransaction {
        bool field; // TODO
    }

    function validate_and_apply(AAPTransaction[] calldata _block) public {}

    function process_erc20_deposits(
        AssetType memory _asset_type,
        uint256 _amount,
        address _sender
    ) private {}

    function insert_record(AssetRecord memory _record) internal {}

    function process_standard_transaction(AAPTransaction memory _transaction)
        internal
    {}

    function process_burn_transaction(AAPTransaction memory _transaction)
        internal
    {}
}
