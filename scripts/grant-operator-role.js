const { ethers } = require('ethers');

async function main() {
  console.log('Starting...');
  const provider = new ethers.JsonRpcProvider('http://localhost:9045');
  const privateKey = '0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae';
  const signer = new ethers.Wallet(privateKey, provider);
  
  // Get Karma contract address from environment variable
  const KARMA = process.env.KARMA_CONTRACT_ADDRESS;
  if (!KARMA) {
    throw new Error('KARMA_CONTRACT_ADDRESS environment variable is not set');
  }
  
  const ADMIN = signer.address;
  
  console.log('Admin:', ADMIN);
  console.log('Karma:', KARMA);
  
  const karmaAbi = [
    'function OPERATOR_ROLE() view returns (bytes32)',
    'function grantRole(bytes32 role, address account)',
    'function hasRole(bytes32 role, address account) view returns (bool)'
  ];
  
  const karma = new ethers.Contract(KARMA, karmaAbi, signer);
  
  // Get the actual OPERATOR_ROLE value from the contract
  const OPERATOR_ROLE = await karma.OPERATOR_ROLE();
  console.log('OPERATOR_ROLE:', OPERATOR_ROLE);
  
  // Check current role
  console.log('Checking current role...');
  const hasRole = await karma.hasRole(OPERATOR_ROLE, ADMIN);
  console.log('Has OPERATOR_ROLE:', hasRole);
  
  if (!hasRole) {
    console.log('Granting OPERATOR_ROLE...');
    const tx = await karma.grantRole(OPERATOR_ROLE, ADMIN, { 
      gasPrice: ethers.parseUnits('15', 'gwei'),
      gasLimit: 100000
    });
    console.log('TX sent:', tx.hash);
    const receipt = await tx.wait();
    console.log('TX confirmed:', receipt.status === 1 ? 'SUCCESS' : 'FAILED');
    
    // Verify
    const nowHasRole = await karma.hasRole(OPERATOR_ROLE, ADMIN);
    console.log('Now has OPERATOR_ROLE:', nowHasRole);
  }
  
  console.log('Done!');
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
