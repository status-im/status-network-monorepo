const { ethers } = require('ethers');

async function main() {
  console.log('Setting up separate prover account...');
  
  const provider = new ethers.JsonRpcProvider('http://localhost:9045');
  const adminKey = '0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae';
  const admin = new ethers.Wallet(adminKey, provider);
  
  // Create a new prover account with a deterministic key
  // Use a fixed key for the prover so it's predictable across restarts
  const proverKey = '0x8f5a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a';
  const prover = new ethers.Wallet(proverKey, provider);
  
  console.log('Admin address:', admin.address);
  console.log('Prover address:', prover.address);
  
  // Fund the prover account
  const proverBalance = await provider.getBalance(prover.address);
  console.log('Prover balance:', ethers.formatEther(proverBalance), 'ETH');
  
  if (proverBalance < ethers.parseEther('10')) {
    console.log('Funding prover account with 100 ETH...');
    const fundTx = await admin.sendTransaction({
      to: prover.address,
      value: ethers.parseEther('100'),
      gasPrice: ethers.parseUnits('15', 'gwei'),
    });
    await fundTx.wait();
    console.log('Prover funded!');
  }
  
  // Get RLN contract address from environment variable
  const RLN = process.env.RLN_CONTRACT_ADDRESS;
  if (!RLN) {
    throw new Error('RLN_CONTRACT_ADDRESS environment variable is not set');
  }
  console.log('RLN contract:', RLN);
  
  const rlnAbi = [
    'function REGISTER_ROLE() view returns (bytes32)',
    'function grantRole(bytes32 role, address account)',
    'function hasRole(bytes32 role, address account) view returns (bool)'
  ];
  
  const rln = new ethers.Contract(RLN, rlnAbi, admin);
  
  // Get the actual REGISTER_ROLE value from the contract
  const REGISTER_ROLE = await rln.REGISTER_ROLE();
  console.log('REGISTER_ROLE:', REGISTER_ROLE);
  
  const hasRole = await rln.hasRole(REGISTER_ROLE, prover.address);
  console.log('Prover has REGISTER_ROLE:', hasRole);
  
  if (!hasRole) {
    console.log('Granting REGISTER_ROLE to prover...');
    const grantTx = await rln.grantRole(REGISTER_ROLE, prover.address, {
      gasPrice: ethers.parseUnits('15', 'gwei'),
    });
    await grantTx.wait();
    console.log('REGISTER_ROLE granted!');
  }
  
  console.log('\n=== Prover Account Setup Complete ===');
  console.log('Prover Private Key:', proverKey);
  console.log('Prover Address:', prover.address);
  console.log('\nUse this private key when starting the RLN prover:');
  console.log(`-e PRIVATE_KEY=${proverKey}`);
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
