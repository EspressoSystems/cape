//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract Wrapper {
    struct Policy {
        bool field; //TODO
    }

    struct AssetType {
        bool field; //TODO
    }

    struct AssetRecord {
        bool field; // TODO
    }

    function credit_erc20_balance(
        address erc20_token,
        uint256 amount,
        address recipient
    ) private {}

    function gen_asset_type_erc20(
        address erc20_token,
        string calldata sponsor,
        Policy calldata policy
    ) public {}

    function withdraw(
        address recipient,
        address erc20_token,
        uint256 amount
    ) public {}

    function wrap_erc20_token(
        AssetType memory asset_type,
        AssetRecord memory record,
        address sender
    ) public {}
}
