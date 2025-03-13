// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BlockhashOpCode {
    bytes32 public lastBlockHash;

    function updateBlockHash() external {
        bytes32 lastBlockHash1 = blockhash(block.number - 1);
        bytes32 lastBlockHash2 = blockhash(block.number - 2);
        lastBlockHash = lastBlockHash1 ^ lastBlockHash2;
    }
}
