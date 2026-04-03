use std::fmt::Formatter;
// third-party
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
    transports::{RpcError, TransportErrorKind},
};
use alloy::primitives::{address, Bytes};
use alloy::providers::MulticallError;
use alloy::sol_types::SolCall;
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum KarmaTiersError {
    #[error("RPC transport error: {0}")]
    RpcTransportError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    Alloy(#[from] alloy::contract::Error),
    #[error(transparent)]
    MultiCall(#[from] MulticallError),
    #[error(transparent)]
    Decode(#[from] alloy::sol_types::Error),
    #[error("Pending transaction error: {0}")]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
    #[error("Private key cannot be empty")]
    EmptyPrivateKey,
    #[error("Unable to connect with signer: {0}")]
    SignerConnectionError(String),
    #[error("Tier count too high (exceeds u8)")]
    TierCountTooHigh,
}

sol!(
    // src: status-network-contracts/src/KarmaTiers.sol
    // Compile bytecode using:
    // docker run -v ./:/sources ethereum/solc:0.8.26 --bin --via-ir --optimize --optimize-runs 1 --overwrite @openzeppelin/contracts=/sources/lib/openzeppelin-contracts/contracts /sources/src/KarmaTiers.sol

    #[sol(rpc, bytecode = "608080604052346059575f8054336001600160a01b0319821681178355916001600160a01b03909116907f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e09080a3610a88908161005e8239f35b5f80fdfe60806040526004361015610011575f80fd5b5f3560e01c8063039af9eb146107335780635e12faa91461071957806367184e28146106fc578063715018a6146106b85780638da5cb5b14610691578063a04f7fc714610668578063c7a416711461058c578063f1180965146101375763f2fde38b1461007c575f80fd5b34610133576020366003190112610133576004356001600160a01b03811690819003610133576100aa6109db565b80156100df575f80546001600160a01b03198116831782556001600160a01b0316905f80516020610a338339815191529080a3005b60405162461bcd60e51b815260206004820152602660248201527f4f776e61626c653a206e6577206f776e657220697320746865207a65726f206160448201526564647265737360d01b6064820152608490fd5b5f80fd5b34610133576020366003190112610133576004356001600160401b0381116101335736602382011215610133576004810135906001600160401b03821161013357602481013660248460051b84010111610133576101936109db565b821561057d576101a38382610978565b3561055857506001545f6001558061049c575b505f9160a219368390030191835b60ff81169083821015610476576024611fe08260051b1684010135858112156101335760249084010191604083016101fc81856109a9565b6001600160401b0381116103d15760405191610222601f8301601f191660200184610829565b818352368282011161013357815f9260209283860137830101528051156104675751602081116104505750602084013597843592838a1061043957806103f8575b50506001548890600160401b8110156103d15780600161028692016001556107a6565b9390936103e557835560018301556102a26002830191856109a9565b906001600160401b0382116103d1576102bb83546107d6565b601f8111610396575b505f90601f831160011461032e57918060039492606096945f92610323575b50508160011b915f1990861b1c19161790555b019201359163ffffffff83168093036101335761031e9263ffffffff19825416179055610910565b6101c4565b013590508c806102e3565b601f19831691845f5260205f20925f5b81811061037e5750926001928592606098966003989610610367575b505050811b0190556102f6565b01355f1983881b60f8161c191690558c808061035a565b9193602060018192878701358155019501920161033e565b6103c190845f5260205f20601f850160051c810191602086106103c7575b601f0160051c0190610993565b8a6102c4565b90915081906103b4565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f525f60045260245ffd5b6001820180921161042557838214610263579091506341ca18b760e01b5f5260045260245260445260645ffd5b634e487b7160e01b5f52601160045260245ffd5b898463773ae66960e11b5f5260045260245260445ffd5b633a49c1e760e21b5f52600452602060245260445ffd5b63820f10d160e01b5f5260045ffd5b7f37740b69a1cce7c6b884ff59b1465c52017ffcb23b6e46249f50f3375b71eada5f80a1005b6001600160fe1b03811681036104255760015f5260021b7fb10e2d527612073b26eecdfd717e6a320cf44b4afac2b0732d9fcbe2b7fa0cf6908101905b8181106104e657506101b6565b805f600492555f60018201556002810161050081546107d6565b9081610515575b50505f6003820155016104d9565b81601f5f931160011461052c5750555b8580610507565b8183526020832061054891601f0160051c810190600101610993565b8082528160208120915555610525565b8261056291610978565b356341ca18b760e01b5f525f6004525f60245260445260645ffd5b637fa68b0560e01b5f5260045ffd5b346101335760203660031901126101335760043560ff8116808203610133575f60606040516105ba8161080e565b82815282602082015281604082015201526001541115610659576105dd906107a6565b506040516105ea8161080e565b815481526001820154916020820192835263ffffffff61064d8160036106126002860161084c565b9460408701958652015416926060850193845260405195869560208752516020870152516040860152516080606086015260a08501906108ec565b91511660808301520390f35b635b8955f760e01b5f5260045ffd5b34610133576020366003190112610133576020610686600435610921565b60ff60405191168152f35b34610133575f366003190112610133575f546040516001600160a01b039091168152602090f35b34610133575f366003190112610133576106d06109db565b5f80546001600160a01b0319811682556001600160a01b03165f80516020610a338339815191528280a3005b34610133575f366003190112610133576020600154604051908152f35b34610133575f366003190112610133576020604051818152f35b34610133576020366003190112610133576004356001548110156101335761075a906107a6565b50805460018201549161079c63ffffffff60036107796002850161084c565b9301541691604051948594855260208501526080604085015260808401906108ec565b9060608301520390f35b6001548110156107c25760015f5260205f209060021b01905f90565b634e487b7160e01b5f52603260045260245ffd5b90600182811c92168015610804575b60208310146107f057565b634e487b7160e01b5f52602260045260245ffd5b91607f16916107e5565b608081019081106001600160401b038211176103d157604052565b601f909101601f19168101906001600160401b038211908210176103d157604052565b9060405191825f82549261085f846107d6565b80845293600181169081156108ca5750600114610886575b5061088492500383610829565b565b90505f9291925260205f20905f915b8183106108ae575050906020610884928201015f610877565b6020919350806001915483858901015201910190918492610895565b90506020925061088494915060ff191682840152151560051b8201015f610877565b805180835260209291819084018484015e5f828201840152601f01601f1916010190565b60ff1660ff81146104255760010190565b6001545f5b60ff8116828110156109645761093b826107a6565b50548410610952575061094d90610910565b610926565b925050505f190160ff81116104255790565b50505f198101915081116104255760ff1690565b90156107c257803590607e1981360301821215610133570190565b81811061099e575050565b5f8155600101610993565b903590601e198136030182121561013357018035906001600160401b0382116101335760200191813603831361013357565b5f546001600160a01b031633036109ee57565b606460405162461bcd60e51b815260206004820152602060248201527f4f776e61626c653a2063616c6c6572206973206e6f7420746865206f776e65726044820152fdfe8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0a2646970667358221220e940ec72bb0432c16107569ad17ab8336bb57be89b40637798ebfa873f7e480b64736f6c634300081a0033")]

    contract KarmaTiers is Ownable {
        /// @notice Emitted when a tier list is updated
        event TiersUpdated();
        /// @notice Emitted when a transaction amount is invalid

        error KarmaTiers__InvalidTxAmount();
        /// @notice Emitted when a tier name is empty
        error KarmaTiers__EmptyTierName();
        /// @notice Emitted when a tier array is empty
        error KarmaTiers__EmptyTiersArray();
        /// @notice Emitted when a tier is not found
        error KarmaTiers__TierNotFound();
        /// @notice Emitted when a tier name exceeds maximum length
        error KarmaTiers__TierNameTooLong(uint256 nameLength, uint256 maxLength);
        /// @notice Emitted when tiers are not contiguous
        error KarmaTiers__NonContiguousTiers(uint8 index, uint256 expectedMinKarma, uint256 actualMinKarma);
        /// @notice Emitted when a tier's minKarma is greater than or equal to maxKarma
        error KarmaTiers__InvalidTierRange(uint256 minKarma, uint256 maxKarma);

        struct Tier {
            uint256 minKarma;
            uint256 maxKarma;
            string name;
            uint32 txPerEpoch;
        }

        uint256 public constant MAX_TIER_NAME_LENGTH = 32;
        Tier[] public tiers;

        function getTierCount() external view returns (uint256 count);
        function getTierById(uint8 tierId) external view onlyValidTierId(tierId) returns (Tier memory tier);
        function updateTiers(Tier[] calldata newTiers) external onlyOwner;
    }
);

impl<P: Provider> KarmaTiers::KarmaTiersInstance<P> {
    pub async fn get_tiers_from_provider(
        provider: &P,
        sc_address: &Address,
    ) -> Result<Vec<Tier>, KarmaTiersError> {
        let karma_tiers_sc = KarmaTiers::new(*sc_address, provider);

        let tier_count = karma_tiers_sc
            .getTierCount()
            .call()
            .await
            .map_err(KarmaTiersError::Alloy)?;

        if tier_count > U256::from(u8::MAX) {
            return Err(KarmaTiersError::TierCountTooHigh);
        }
        // Note: unwrap safe - just tested
        let tier_count = u8::try_from(tier_count).unwrap();

        let mut tiers = Vec::with_capacity(usize::from(tier_count));
        for i in 0..tier_count {
            let tier = karma_tiers_sc
                .getTierById(i)
                .call()
                .await
                .map_err(KarmaTiersError::Alloy)?;
            tiers.push(Tier::from(tier));
        }
        Ok(tiers)
    }

    /// Faster version of `get_tiers_from_provider` (using multicall3)
    pub async fn get_tiers_from_provider_2(
        provider: &P,
        sc_address: &Address,
    ) -> Result<Vec<Tier>, KarmaTiersError> {

        let karma_tiers_sc = KarmaTiers::new(*sc_address, provider);

        let tier_count = karma_tiers_sc
            .getTierCount()
            .call()
            .await
            .map_err(KarmaTiersError::Alloy)?;

        if tier_count > U256::from(u8::MAX) {
            return Err(KarmaTiersError::TierCountTooHigh);
        }
        // Note: unwrap safe - just tested
        let tier_count = u8::try_from(tier_count).unwrap();

        // From example: https://alloy.rs/examples/providers/multicall
        let mut multicall = provider.multicall().dynamic::<KarmaTiers::getTierByIdCall>();
        for i in 0..tier_count {
            multicall = multicall.add_dynamic(karma_tiers_sc.getTierById(i));
        }

        let results = multicall
            .aggregate()
            .await
            .map_err(KarmaTiersError::MultiCall)?;

        let tiers = results.into_iter().map(Tier::from).collect();

        Ok(tiers)
    }

    pub async fn get_tiers_from_provider_3(
        provider: &P,
        sc_address: &Address,
    ) -> Result<Vec<Tier>, KarmaTiersError> {
        let karma_tiers_sc = KarmaTiers::new(*sc_address, provider);

        let tier_count = karma_tiers_sc
            .getTierCount()
            .call()
            .await
            .map_err(KarmaTiersError::Alloy)?;

        if tier_count > U256::from(u8::MAX) {
            return Err(KarmaTiersError::TierCountTooHigh);
        }
        // Note: unwrap safe - just tested
        let tier_count = u8::try_from(tier_count).unwrap();

        let mut calls = Vec::with_capacity(usize::from(tier_count));

        for i in 0..tier_count {
            // Generate the ABI-encoded call data
            // Note: Use the exact generated SolCall struct name for your method
            let call_struct = KarmaTiers::getTierByIdCall { tierId: i };
            let encoded_calldata = call_struct.abi_encode();

            calls.push(IMulticall3::Call {
                target: *sc_address,
                callData: Bytes::from(encoded_calldata),
            });
        }

        // From https://www.multicall3.com/ - always using the following address
        let multicall3_address = address!("cA11bde05977b3631167028862bE2a173976CA11");
        let multicall_sc = IMulticall3::new(multicall3_address, provider);

        // 4. Execute the aggregate function on-chain
        let response = multicall_sc
            .aggregate(calls)
            .call()
            .await
            .map_err(KarmaTiersError::Alloy)?;

        // 5. Decode the raw bytes back into your Tier structs
        let mut tiers = Vec::with_capacity(usize::from(tier_count));

        for raw_bytes in response.returnData {
            // Manually decode the bytes
            let decoded_return = KarmaTiers::getTierByIdCall::abi_decode_returns(&raw_bytes)
                .map_err(KarmaTiersError::Decode)?;
            tiers.push(Tier::from(decoded_return));
        }

        Ok(tiers)
    }
}

sol! {
    #[sol(rpc)]
    interface IMulticall3 {
        struct Call {
            address target;
            bytes callData;
        }

        function aggregate(Call[] calldata calls)
            external
            view
            returns (uint256 blockNumber, bytes[] memory returnData);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tier {
    pub min_karma: U256,
    pub max_karma: U256,
    pub name: String,
    pub tx_per_epoch: u32,
}

impl From<KarmaTiers::Tier> for Tier {
    fn from(value: KarmaTiers::Tier) -> Self {
        Self {
            min_karma: value.minKarma,
            max_karma: value.maxKarma,
            name: value.name,
            tx_per_epoch: value.txPerEpoch,
        }
    }
}

impl From<KarmaTiers::tiersReturn> for Tier {
    fn from(tiers_return: KarmaTiers::tiersReturn) -> Self {
        Self {
            min_karma: tiers_return._0,
            max_karma: tiers_return._1,
            name: tiers_return._2,
            tx_per_epoch: tiers_return._3,
            // active: tiers_return._4,
        }
    }
}

impl std::fmt::Debug for KarmaTiers::Tier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "KarmaTiers::Tier min_karma: {}, max_karma: {}, name: {}, tx_per_epoch: {}",
            self.minKarma, self.maxKarma, self.name, self.txPerEpoch
        )
    }
}

#[cfg(feature = "anvil")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::KarmaTiers::KarmaTiersInstance;
    use alloy::{
        providers::ProviderBuilder,
        node_bindings::Anvil,
        network::EthereumWallet,
        signers::local::PrivateKeySigner
    };

    impl PartialEq<KarmaTiers::Tier> for Tier {
        fn eq(&self, other: &KarmaTiers::Tier) -> bool {
            self.min_karma == other.minKarma
                && self.max_karma == other.maxKarma
                && self.name == other.name
                && self.tx_per_epoch == other.txPerEpoch
        }
    }

    impl PartialEq for KarmaTiers::Tier {
        fn eq(&self, other: &Self) -> bool {
            self.minKarma == other.minKarma
                && self.maxKarma == other.maxKarma
                && self.name == other.name
                && self.txPerEpoch == other.txPerEpoch
        }
    }

    #[tokio::test]
    async fn test_get_tiers() {

        tracing_subscriber::fmt::init();

        // Spawn anvil using a fork of Sepolia (this to have the Multicall3 contract deployed)
        let anvil = Anvil::new()
            .fork("https://ethereum-sepolia-rpc.publicnode.com")
            .spawn();

        let provider = {
            let anvil_priv_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let signer: PrivateKeySigner = anvil_priv_key.parse().unwrap();
            let wallet = EthereumWallet::from(signer);

            ProviderBuilder::new()
                .wallet(wallet)
                .connect_http(anvil.endpoint().parse().unwrap())
        };

        let code = provider.get_code_at(address!("cA11bde05977b3631167028862bE2a173976CA11")).await.unwrap();
        // println!("Multicall3 Bytecode: {:?}", code);
        if code.is_empty() {
            panic!("No Multicall3 smart contract deployed?");
        }

        // Deploy the KarmaTiers contract.
        let contract = KarmaTiers::deploy(&provider).await.unwrap();

        // getTierCount call
        let call_1 = contract.getTierCount();
        let result_1 = call_1.call().await.unwrap();
        assert_eq!(result_1, U256::from(0));

        // updateTiers call

        let tiers = [
            KarmaTiers::Tier {
                minKarma: U256::from(0),
                maxKarma: U256::from(99),
                name: "Basic".to_string(),
                txPerEpoch: 10,
            },
            KarmaTiers::Tier {
                minKarma: U256::from(100),
                maxKarma: U256::from(499),
                name: "Advanced".to_string(),
                txPerEpoch: 50,
            },
        ];

        let call_2 = contract.updateTiers(tiers.to_vec());
        let _tx_hash = call_2.send().await.unwrap().watch().await.unwrap();
        // let result_2 = call_2.call().await.unwrap();

        let call_3 = contract.getTierCount();
        let result_3 = call_3.call().await.unwrap();
        assert_eq!(result_3, U256::from(tiers.len()));

        let call_4 = contract.getTierById(0);
        let result_4 = call_4.call().await.unwrap();
        assert_eq!(result_4, tiers[0]);

        let call_5 = contract.getTierById(1);
        let result_5 = call_5.call().await.unwrap();
        assert_eq!(result_5, tiers[1]);

        println!("Now calling get_tiers_from_provider...");
        let res = KarmaTiersInstance::get_tiers_from_provider(&provider, contract.address())
            .await
            .unwrap();
        assert_eq!(res, tiers.to_vec());

        println!("Now calling get_tiers_from_provider_2...");
        let res_2 = KarmaTiersInstance::get_tiers_from_provider_2(&provider, contract.address())
            .await
            .unwrap();
        assert_eq!(res_2, tiers.to_vec());

        println!("Now calling get_tiers_from_provider_3...");
        let res_3 = KarmaTiersInstance::get_tiers_from_provider_3(&provider, contract.address())
            .await
            .unwrap();
        assert_eq!(res_3, tiers.to_vec());

    }

}
