// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

/**
 * @title IGaugeVoter
 * @notice Interface for contracts that track voting power changes in gauge voting.
 */
interface IGaugeVoter {
    /**
     * @notice Called when voting power changes due to token transfers or delegation.
     * @param from The address losing voting power.
     * @param to The address gaining voting power.
     */
    function updateVotingPower(address from, address to) external;
}
