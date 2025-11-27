import { ethers } from "ethers";
import { RLN_CONFIG } from "./rln-config";

// Karma Contract ABI - includes all methods needed for testing
const KARMA_ABI = [
  "function balanceOf(address account) view returns (uint256)",
  "function mint(address to, uint256 amount)",
  "function totalSupply() view returns (uint256)",
  "function name() view returns (string)",
  "function symbol() view returns (string)",
  "function decimals() view returns (uint8)",
  "function owner() view returns (address)",
  "function hasRole(bytes32 role, address account) view returns (bool)",
  "function MINTER_ROLE() view returns (bytes32)",
  "event Transfer(address indexed from, address indexed to, uint256 value)",
  "event KarmaMinted(address indexed to, uint256 amount)",
];

// RLN Membership Contract ABI
// Based on actual RLN.sol contract: mapping(uint256 commitment => User user) public members
// where User = { address userAddress, uint256 index }
const RLN_ABI = [
  "function register(uint256 identityCommitment, address user)",
  "function members(uint256 commitment) view returns (address userAddress, uint256 index)",
  "function identityCommitmentIndex() view returns (uint256)",
  "function SET_SIZE() view returns (uint256)",
  "function karma() view returns (address)",
  "function hasRole(bytes32 role, address account) view returns (bool)",
  "event MemberRegistered(uint256 identityCommitment, uint256 index)",
  "event MemberSlashed(uint256 index, address slasher)",
];

// Karma Tiers Contract ABI
const KARMA_TIERS_ABI = [
  "function getTier(uint256 karmaBalance) view returns (tuple(string name, uint256 minKarma, uint256 maxKarma, uint32 txPerEpoch))",
  "function getTierByName(string name) view returns (tuple(string name, uint256 minKarma, uint256 maxKarma, uint32 txPerEpoch))",
  "function getAllTiers() view returns (tuple(string name, uint256 minKarma, uint256 maxKarma, uint32 txPerEpoch)[])",
  "function updateTiers(tuple(string name, uint256 minKarma, uint256 maxKarma, uint32 txPerEpoch)[] tiers)",
  "function tierCount() view returns (uint256)",
  "event TiersUpdated()",
];

// Stake Manager Contract ABI (for slashing tests)
const STAKE_MANAGER_ABI = [
  "function stake(address user) view returns (uint256)",
  "function slash(address user, uint256 amount)",
  "function deposit(uint256 amount)",
  "function withdraw(uint256 amount)",
  "event Slashed(address indexed user, uint256 amount)",
  "event Deposited(address indexed user, uint256 amount)",
];

export interface RlnContracts {
  karma: ethers.Contract;
  rln: ethers.Contract;
  karmaTiers: ethers.Contract;
  stakeManager: ethers.Contract;
}

/**
 * Load deployed RLN contracts
 */
export function loadRlnContracts(provider: ethers.Provider, signer?: ethers.Signer): RlnContracts {
  const providerOrSigner = signer || provider;

  return {
    karma: new ethers.Contract(RLN_CONFIG.contracts.karma, KARMA_ABI, providerOrSigner),
    rln: new ethers.Contract(RLN_CONFIG.contracts.rln, RLN_ABI, providerOrSigner),
    karmaTiers: new ethers.Contract(RLN_CONFIG.contracts.karmaTiers, KARMA_TIERS_ABI, providerOrSigner),
    stakeManager: new ethers.Contract(RLN_CONFIG.contracts.stakeManager, STAKE_MANAGER_ABI, providerOrSigner),
  };
}

/**
 * Get contract addresses
 */
export function getContractAddresses() {
  return RLN_CONFIG.contracts;
}

/**
 * Verify all contracts are deployed and accessible
 */
export async function verifyContracts(contracts: RlnContracts): Promise<boolean> {
  try {
    // Check Karma contract
    await contracts.karma.totalSupply();

    // Check RLN contract
    await contracts.rln.memberCount();

    // Check KarmaTiers contract
    await contracts.karmaTiers.tierCount();

    return true;
  } catch (error) {
    console.error("Contract verification failed:", error);
    return false;
  }
}
