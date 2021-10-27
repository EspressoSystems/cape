//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./Wrapper.sol";

contract AAPE is Wrapper {
    struct AAPTransaction {
        bool field; // TODO
    }

    function validate_and_apply(AAPTransaction[] calldata block) public {}

    function process_erc20_deposits(
        AssetType memory asset_type,
        uint256 amount,
        address sender
    ) private {}

    function insert_record(AssetRecord memory record) private {}

    function process_standard_transaction(AAPTransaction memory transaction)
        private
    {}

    function process_burn_transaction(AAPTransaction memory transaction)
        private
    {}
}
