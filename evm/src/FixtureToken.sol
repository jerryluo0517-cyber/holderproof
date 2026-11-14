// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

/// Test-only ERC-20 ledger. Replays fixture transfers without impersonating
/// holders. This is a correctness oracle, not a production token contract.
contract FixtureToken {
    address public immutable owner = msg.sender;
    mapping(address => uint256) public balanceOf;
    uint256 public totalSupply;
    event Transfer(address indexed from, address indexed to, uint256 value);

    function replay(address[] calldata from, address[] calldata to, uint256[] calldata amount) external {
        require(msg.sender == owner, "owner only");
        require(from.length == to.length && to.length == amount.length, "length mismatch");
        for (uint256 i; i < amount.length; ++i) {
            if (from[i] == address(0)) totalSupply += amount[i];
            else balanceOf[from[i]] -= amount[i];
            if (to[i] == address(0)) totalSupply -= amount[i];
            else balanceOf[to[i]] += amount[i];
            emit Transfer(from[i], to[i], amount[i]);
        }
    }
}
