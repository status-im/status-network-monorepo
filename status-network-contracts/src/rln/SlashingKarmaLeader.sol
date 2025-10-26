// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title SlashingKarmaLeader
/// @notice Periodic leader election with stake-weighted lottery using non-transferable Karma.
/// @dev
/// - Time is split into fixed-length block windows ("periods").
/// - Anyone can register by snapshotting Karma; snapshots are per (account, period).
/// - Selection uses commit-reveal and period seed = blockhash(startBlock(period)).
/// - Score = hash(seed, account, salt) / karmaSnapshot[account][period]; lowest score wins; ties: smaller address
/// wins. - Automatic snapshots:
///     (a) commit(): if missing snapshot for {msg.sender, period}, take it immediately (must be before start);
///     (b) reveal(): if missing snapshot for {msg.sender, period+1} and before its start, take it (pre-snapshot).
/// - Gas/scalability: O(1) per participant operation (no loops over all watchers).
/// - IMPORTANT: blockhash is only available for the most recent 256 blocks; ensure periodLength ≤ 256 so reveal
/// happens while blockhash(startBlock) is retrievable. If you need longer periods, swap the randomness source
/// accordingly.
interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
}

contract SlashingKarmaLeader {
    IERC20 public immutable karmaToken;
    uint256 public immutable periodLength; // in blocks, MUST be ≤ 256
    uint256 public immutable firstPeriodStart; // block number

    mapping(address => bool) public isWatcher;
    mapping(address => mapping(uint256 => uint256)) public karmaSnapshot; // snapshot per account/period
    mapping(uint256 => mapping(address => bytes32)) public commitments; // per period/account
    mapping(uint256 => mapping(address => bool)) public revealed; // per period/account
    mapping(uint256 => address) public leaderOf; // period winner
    mapping(uint256 => uint256) public bestScore; // period best score

    event WatcherRegistered(address indexed watcher);
    event KarmaSnapshotted(address indexed watcher, uint256 indexed period, uint256 karma);
    event CommitSubmitted(address indexed watcher, uint256 indexed period, bytes32 commitment);
    event RevealSubmitted(address indexed watcher, uint256 indexed period, uint256 salt, uint256 score);
    event LeaderUpdated(uint256 indexed period, address indexed leader, uint256 score);

    /// @param karmaTokenAddress Karma token
    /// @param _periodLength Period size in blocks. Keep ≤ 256 to safely use blockhash(startBlock).
    constructor(address karmaTokenAddress, uint256 _periodLength) {
        require(karmaTokenAddress != address(0), "karma=0");
        require(_periodLength > 0 && _periodLength <= 256, "periodLength invalid");
        karmaToken = IERC20(karmaTokenAddress);
        periodLength = _periodLength;
        firstPeriodStart = block.number + _periodLength; // allow initial commit phase for period=1
    }

    /// @notice Start block (inclusive) for a given 1-indexed period.
    function periodStart(uint256 period) public view returns (uint256) {
        require(period >= 1, "period>=1");
        return firstPeriodStart + (period - 1) * periodLength;
    }

    /// @notice End block (exclusive) for a period.
    function periodEnd(uint256 period) public view returns (uint256) {
        return periodStart(period) + periodLength;
    }

    /// @notice Seed for a period (blockhash of start block).
    function periodSeed(uint256 period) public view returns (bytes32) {
        return blockhash(periodStart(period));
    }

    /// @notice Manually snapshot `account`'s Karma for the next not-yet-started period.
    /// @dev Anyone may call this; it also registers the watcher on first snapshot.
    function updateKarma(address account) external {
        uint256 period = _nextSnapshotPeriod();
        _snapshot(account, period);
    }

    /// @dev Computes the target period for next snapshot, ensuring it's not started yet.
    function _nextSnapshotPeriod() internal view returns (uint256 period) {
        if (block.number < firstPeriodStart) return 1;
        uint256 elapsed = block.number - firstPeriodStart;
        uint256 idx = elapsed / periodLength + 1; // current period (if started)
        uint256 start = periodStart(idx);
        period = (block.number >= start) ? (idx + 1) : idx; // snapshot current if not started, else next
    }

    /// @dev Takes a snapshot for {account, period}; reverts if period already started.
    function _snapshot(address account, uint256 period) internal {
        uint256 start = periodStart(period);
        require(block.number < start, "period started");
        uint256 bal = karmaToken.balanceOf(account);
        karmaSnapshot[account][period] = bal;
        if (!isWatcher[account]) {
            isWatcher[account] = true;
            emit WatcherRegistered(account);
        }
        emit KarmaSnapshotted(account, period, bal);
    }

    /// @notice Commit a secret salt hash for `period` before it starts.
    /// @param period 1-indexed period
    /// @param commitment keccak256(abi.encodePacked(period, msg.sender, salt))
    function commit(uint256 period, bytes32 commitment) external {
        uint256 start = periodStart(period);
        require(block.number < start, "commit phase ended");
        if (karmaSnapshot[msg.sender][period] == 0) {
            _snapshot(msg.sender, period); // auto snapshot for this period
        }
        require(karmaSnapshot[msg.sender][period] > 0, "karma=0");
        require(commitments[period][msg.sender] == bytes32(0), "already committed");
        commitments[period][msg.sender] = commitment;
        emit CommitSubmitted(msg.sender, period, commitment);
    }

    /*===============================  REVEAL  =============================*/

    /// @notice Reveal `salt` for `period` during the period; updates leader if this score is better (lower).
    /// @param period 1-indexed period
    /// @param salt The secret committed as keccak256(period, msg.sender, salt)
    function reveal(uint256 period, uint256 salt) external {
        uint256 start = periodStart(period);
        uint256 end = start + periodLength;
        require(block.number >= start, "reveal too early");
        require(block.number < end, "reveal ended");

        bytes32 c = commitments[period][msg.sender];
        require(c != bytes32(0), "no commit");
        require(!revealed[period][msg.sender], "already revealed");
        bytes32 expect = keccak256(abi.encodePacked(period, msg.sender, salt));
        require(expect == c, "bad salt");

        uint256 karma = karmaSnapshot[msg.sender][period];
        require(karma > 0, "no snapshot karma");

        bytes32 seed = blockhash(start);
        require(seed != bytes32(0), "seed unavailable");
        uint256 rnd = uint256(keccak256(abi.encodePacked(seed, msg.sender, salt)));
        uint256 score = rnd / karma;

        revealed[period][msg.sender] = true;
        emit RevealSubmitted(msg.sender, period, salt, score);

        address cur = leaderOf[period];
        if (cur == address(0)) {
            leaderOf[period] = msg.sender;
            bestScore[period] = score;
            emit LeaderUpdated(period, msg.sender, score);
        } else {
            uint256 best = bestScore[period];
            if (score < best || (score == best && msg.sender < cur)) {
                leaderOf[period] = msg.sender;
                bestScore[period] = score;
                emit LeaderUpdated(period, msg.sender, score);
            }
        }

        uint256 next = period + 1;
        if (karmaSnapshot[msg.sender][next] == 0 && block.number < periodStart(next)) {
            _snapshot(msg.sender, next);
        }
    }
}
