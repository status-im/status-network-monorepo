// SPDX-License-Identifier: Apache-2.0 OR MIT
pragma solidity 0.8.26;

import { Karma } from "../Karma.sol";
import { IPoseidonHasher } from "./PoseidonHasher.sol";

import { AccessControlUpgradeable } from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import { Initializable } from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import { UUPSUpgradeable } from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

/// @title Rate-Limiting Nullifier registry contract
/// @dev This contract allows you to register RLN commitment and withdraw/slash.
contract RLN is Initializable, UUPSUpgradeable, AccessControlUpgradeable {
    error RLN__MemberNotFound();
    error RLN__IdCommitmentAlreadyRegistered();
    error RLN__Unauthorized();
    error RLN__InvalidCommitment();
    error RLN__RevealWindowNotStarted();
    error RLN__InvalidSlashRevealWindowTime(uint256 windowTime);

    /// @dev Emmited when a new member registered.
    /// @param identityCommitment: `identityCommitment`;
    /// @param index: idCommitmentIndex value.
    event MemberRegistered(uint256 identityCommitment, uint256 index);

    /// @dev Emmited when a member was slashed.
    /// @param index: index of `identityCommitment`;
    /// @param slasher: address of slasher (msg.sender).
    event MemberSlashed(uint256 index, address slasher);

    /// @dev Emitted when the slash reveal window time is updated.
    /// @param newWindowTime: the new reveal window time in seconds.
    /// @param updatedBy: address of the account that performed the update.
    event SlashRevealWindowTimeUpdated(uint256 newWindowTime, address indexed updatedBy);

    /// @dev User metadata struct.
    /// @param userAddress: address of depositor;
    struct User {
        address userAddress;
        uint256 index;
    }

    bytes32 public constant SLASHER_ROLE = keccak256("SLASHER_ROLE");
    bytes32 public constant REGISTER_ROLE = keccak256("REGISTER_ROLE");

    /// @dev Current index where identityCommitment will be stored.
    uint256 public identityCommitmentIndex;

    /// @dev Registry set. The keys are `identityCommitment`s.
    /// The values are addresses of accounts that call `register` transaction.
    mapping(uint256 commitment => User user) public members;

    /// @dev Karma Token used for registering.
    Karma public karma;

    /// @dev Poseidon hasher contract.
    IPoseidonHasher public poseidonHasher;

    /// @dev Time window for reveal after commit (default 1 hour).
    uint256 public slashRevealWindowTime;

    /// @dev Last reveal start time for each account to be slashed.
    mapping(address account => uint256 lastRevealStartTime) public lastRevealStartTime;

    /// @dev Slash commitments mapping for the commit-reveal scheme.
    /// Maps account => commitmentHash => revealStartTime.
    mapping(address account => mapping(bytes32 hash => uint256 revealStartTime)) public slashCommitments;

    constructor() {
        _disableInitializers();
    }

    /// @dev Constructor.
    /// @param _owner: address of the owner of the contract;
    /// @param _slasher: address of the slasher;
    /// @param _register: address of the register;
    /// @param _token: address of the ERC20 contract;
    /// @param _poseidonHasher: address of the PoseidonHasher contract;
    function initialize(
        address _owner,
        address _slasher,
        address _register,
        address _token,
        address _poseidonHasher
    )
        public
        initializer
    {
        __UUPSUpgradeable_init();
        __AccessControl_init();
        _setupRole(DEFAULT_ADMIN_ROLE, _owner);
        _setupRole(SLASHER_ROLE, _slasher);
        _setupRole(REGISTER_ROLE, _register);
        /// forge-lint: disable-next-line(incorrect-shift)

        karma = Karma(_token);
        poseidonHasher = IPoseidonHasher(_poseidonHasher);

        // Set default reveal window time to 1 hour
        slashRevealWindowTime = 1 hours;
    }

    /**
     * @notice Authorizes contract upgrades via UUPS.
     * @dev This function is only callable by the owner.
     */
    function _authorizeUpgrade(address) internal view override {
        if (!hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert RLN__Unauthorized();
        }
    }

    /// @dev Computes the slash commitment key with the slasher address and hash.
    /// The slasher address is included to ensure uniqueness per slasher, not allowing anyone else to override the same
    /// hash without even knowing the private key.
    /// @param sender: address of the slasher;
    /// @param hash: keccak256 hash of abi.encodePacked(privateKey, rewardRecipient);
    /// @return bytes32: the computed commitment key.
    function _slashCommitmentKey(address sender, bytes32 hash) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(sender, hash));
    }

    /// @dev Sets the slash reveal window time.
    /// @param _slashRevealWindowTime: new reveal window time in seconds.
    /// @notice The window time must be at least 1 second and no more than 1 day.
    ///         A non-zero value is required to ensure the queuing mechanism functions correctly.
    ///         An excessively large value could lock commitments indefinitely.
    function setSlashRevealWindowTime(uint256 _slashRevealWindowTime) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (_slashRevealWindowTime == 0 || _slashRevealWindowTime > 1 days) {
            revert RLN__InvalidSlashRevealWindowTime(_slashRevealWindowTime);
        }

        slashRevealWindowTime = _slashRevealWindowTime;
        emit SlashRevealWindowTimeUpdated(_slashRevealWindowTime, msg.sender);
    }

    /// @dev Adds `identityCommitment` to the registry set and takes the necessary stake amount.
    ///
    /// NOTE: The set must not be full.
    ///
    /// @param identityCommitment: `identityCommitment`;
    function register(uint256 identityCommitment, address user) external onlyRole(REGISTER_ROLE) {
        uint256 index = identityCommitmentIndex;
        if (members[identityCommitment].userAddress != address(0)) {
            revert RLN__IdCommitmentAlreadyRegistered();
        }

        /// forge-lint: disable-next-line(named-struct-fields)
        members[identityCommitment] = User(user, index);
        emit MemberRegistered(identityCommitment, index);

        unchecked {
            identityCommitmentIndex = index + 1;
        }
    }

    /// @dev Commits to a future slash operation using a hash.
    /// @notice This is the first step of the commit-reveal scheme for slashing.
    /// The slasher must first commit to a hash of (privateKey, rewardRecipient) before revealing
    /// the actual values. This prevents front-running attacks where others could observe the
    /// privateKey in the mempool and slash the member themselves with a different reward recipient.
    /// The commit is queued by account, with each new commit having a reveal window after the previous one.
    ///
    /// @param account: the account to be slashed (address associated with the identity commitment).
    /// @param hash: keccak256 hash of abi.encodePacked(privateKey, rewardRecipient).
    function slashCommit(address account, bytes32 hash) external onlyRole(SLASHER_ROLE) {
        uint256 lastReveal = lastRevealStartTime[account];
        uint256 revealStartTime;

        if (lastReveal == 0 || lastReveal + slashRevealWindowTime < block.timestamp) {
            revealStartTime = block.timestamp;
        } else {
            revealStartTime = lastReveal + slashRevealWindowTime;
        }

        bytes32 key = _slashCommitmentKey(msg.sender, hash);
        slashCommitments[account][key] = revealStartTime;
        lastRevealStartTime[account] = revealStartTime;
    }

    /// @dev Reveals and executes a previously committed slash operation.
    /// @notice This is the second step of the commit-reveal scheme for slashing.
    /// After committing the hash, the slasher reveals the actual privateKey and rewardRecipient.
    /// The function verifies that these values match a previously committed hash and that the
    /// reveal window has started. This two-step process with queuing prevents front-running
    /// while ensuring the slasher cannot change the parameters after commitment.
    ///
    /// @param account: the account to be slashed (address associated with the identity commitment).
    /// @param privateKey: RLN private key as bytes32.
    /// @param rewardRecipient: Address that will receive the slash reward from the Karma contract.
    function slashReveal(
        address account,
        bytes32 privateKey,
        address rewardRecipient
    )
        external
        onlyRole(SLASHER_ROLE)
    {
        /// forge-lint: disable-next-line(asm-keccak256)
        bytes32 hash = keccak256(abi.encodePacked(privateKey, rewardRecipient));
        bytes32 key = _slashCommitmentKey(msg.sender, hash);
        uint256 revealStartTime = slashCommitments[account][key];

        if (revealStartTime == 0) {
            revert RLN__InvalidCommitment();
        }

        if (block.timestamp < revealStartTime) {
            revert RLN__RevealWindowNotStarted();
        }

        delete slashCommitments[account][key];

        uint256 identityCommitment = poseidonHasher.hash(uint256(privateKey));
        User memory member = members[identityCommitment];
        if (member.userAddress == address(0)) {
            revert RLN__MemberNotFound();
        }

        // We make sure that the account slashed matches the account used during the commit phase
        // otherwise someone could front-run the slasher by committing to slash a different account
        // to skip the queuing mechanism.
        if (account != member.userAddress) {
            revert RLN__InvalidCommitment();
        }

        karma.slash(member.userAddress, rewardRecipient);
        delete members[identityCommitment];

        emit MemberSlashed(member.index, msg.sender);
    }
}
