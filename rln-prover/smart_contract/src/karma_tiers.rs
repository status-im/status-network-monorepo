use std::fmt::Formatter;
// third-party
use alloy::providers::Provider;
use alloy::{
    primitives::{Address, U256},
    sol,
    transports::{RpcError, TransportErrorKind},
};
// internal
// use crate::common::AlloyWsProvider;

#[derive(thiserror::Error, Debug)]
pub enum KarmaTiersError {
    #[error("RPC transport error: {0}")]
    RpcTransportError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    Alloy(#[from] alloy::contract::Error),
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
    // https://github.com/vacp2p/staking-reward-streamer/pull/224
    // Compile bytecode using:
    // docker run -v ./:/sources ethereum/solc:stable --bin --via-ir --optimize --optimize-runs 1 --overwrite @openzeppelin/contracts=/sources/lib/openzeppelin-contracts/contracts /sources/src/KarmaTiers.sol

    #[sol(rpc, bytecode = "608080604052346100da57610013336100de565b5f54336001600160a01b039091160361009857331561004457610035336100de565b604051610a8990816101258239f35b60405162461bcd60e51b815260206004820152602660248201527f4f776e61626c653a206e6577206f776e657220697320746865207a65726f206160448201526564647265737360d01b6064820152608490fd5b62461bcd60e51b815260206004820181905260248201527f4f776e61626c653a2063616c6c6572206973206e6f7420746865206f776e65726044820152606490fd5b5f80fd5b5f80546001600160a01b039283166001600160a01b03198216811783559216907f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e09080a356fe60806040526004361015610011575f80fd5b5f3560e01c8063039af9eb146107345780635e12faa91461071a57806367184e28146106fd578063715018a6146106b95780638da5cb5b14610692578063a04f7fc714610669578063c7a416711461058d578063f1180965146101375763f2fde38b1461007c575f80fd5b34610133576020366003190112610133576004356001600160a01b03811690819003610133576100aa6109dc565b80156100df575f80546001600160a01b03198116831782556001600160a01b0316905f516020610a345f395f51905f529080a3005b60405162461bcd60e51b815260206004820152602660248201527f4f776e61626c653a206e6577206f776e657220697320746865207a65726f206160448201526564647265737360d01b6064820152608490fd5b5f80fd5b34610133576020366003190112610133576004356001600160401b0381116101335736602382011215610133576004810135906001600160401b03821161013357602481013660248460051b84010111610133576101936109dc565b821561057e576101a38382610979565b3561055957506001545f6001558061049d575b505f9160a219368390030191835b60ff81169083821015610477576024611fe08260051b1684010135858112156101335760249084010191604083016101fc81856109aa565b6001600160401b0381116103d25760405191610222601f8301601f19166020018461082a565b818352368282011161013357815f9260209283860137830101528051156104685751602081116104515750602084013597843592838a111561043a57806103f9575b50506001548890600160401b8110156103d25780600161028792016001556107a7565b9390936103e657835560018301556102a36002830191856109aa565b906001600160401b0382116103d2576102bc83546107d7565b601f8111610397575b505f90601f831160011461032f57918060039492606096945f92610324575b50508160011b915f1990861b1c19161790555b019201359163ffffffff83168093036101335761031f9263ffffffff19825416179055610911565b6101c4565b013590508c806102e4565b601f19831691845f5260205f20925f5b81811061037f5750926001928592606098966003989610610368575b505050811b0190556102f7565b01355f1983881b60f8161c191690558c808061035b565b9193602060018192878701358155019501920161033f565b6103c290845f5260205f20601f850160051c810191602086106103c8575b601f0160051c0190610994565b8a6102c5565b90915081906103b5565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f525f60045260245ffd5b600182018092116104265783821461026457909150639026ca5960e01b5f5260045260245260445260645ffd5b634e487b7160e01b5f52601160045260245ffd5b89846308a0f77b60e31b5f5260045260245260445ffd5b6307b53eff60e51b5f52600452602060245260445ffd5b637e15dcc760e01b5f5260045ffd5b7f37740b69a1cce7c6b884ff59b1465c52017ffcb23b6e46249f50f3375b71eada5f80a1005b6001600160fe1b03811681036104265760015f5260021b7fb10e2d527612073b26eecdfd717e6a320cf44b4afac2b0732d9fcbe2b7fa0cf6908101905b8181106104e757506101b6565b805f600492555f60018201556002810161050181546107d7565b9081610516575b50505f6003820155016104da565b81601f5f931160011461052d5750555b8580610508565b8183526020832061054991601f0160051c810190600101610994565b8082528160208120915555610526565b8261056391610979565b35639026ca5960e01b5f525f6004525f60245260445260645ffd5b630b59608360e21b5f5260045ffd5b346101335760203660031901126101335760043560ff8116808203610133575f60606040516105bb8161080f565b8281528260208201528160408201520152600154111561065a576105de906107a7565b506040516105eb8161080f565b815481526001820154916020820192835263ffffffff61064e8160036106136002860161084d565b9460408701958652015416926060850193845260405195869560208752516020870152516040860152516080606086015260a08501906108ed565b91511660808301520390f35b63b4bcd5a960e01b5f5260045ffd5b34610133576020366003190112610133576020610687600435610922565b60ff60405191168152f35b34610133575f366003190112610133575f546040516001600160a01b039091168152602090f35b34610133575f366003190112610133576106d16109dc565b5f80546001600160a01b0319811682556001600160a01b03165f516020610a345f395f51905f528280a3005b34610133575f366003190112610133576020600154604051908152f35b34610133575f366003190112610133576020604051818152f35b34610133576020366003190112610133576004356001548110156101335761075b906107a7565b50805460018201549161079d63ffffffff600361077a6002850161084d565b9301541691604051948594855260208501526080604085015260808401906108ed565b9060608301520390f35b6001548110156107c35760015f5260205f209060021b01905f90565b634e487b7160e01b5f52603260045260245ffd5b90600182811c92168015610805575b60208310146107f157565b634e487b7160e01b5f52602260045260245ffd5b91607f16916107e6565b608081019081106001600160401b038211176103d257604052565b601f909101601f19168101906001600160401b038211908210176103d257604052565b9060405191825f825492610860846107d7565b80845293600181169081156108cb5750600114610887575b506108859250038361082a565b565b90505f9291925260205f20905f915b8183106108af575050906020610885928201015f610878565b6020919350806001915483858901015201910190918492610896565b90506020925061088594915060ff191682840152151560051b8201015f610878565b805180835260209291819084018484015e5f828201840152601f01601f1916010190565b60ff1660ff81146104265760010190565b6001545f5b60ff8116828110156109655761093c826107a7565b50548410610953575061094e90610911565b610927565b925050505f190160ff81116104265790565b50505f198101915081116104265760ff1690565b90156107c357803590607e1981360301821215610133570190565b81811061099f575050565b5f8155600101610994565b903590601e198136030182121561013357018035906001600160401b0382116101335760200191813603831361013357565b5f546001600160a01b031633036109ef57565b606460405162461bcd60e51b815260206004820152602060248201527f4f776e61626c653a2063616c6c6572206973206e6f7420746865206f776e65726044820152fdfe8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0a26469706673582212208f41e7c9492705a355026a69ee93fdb152512cb904e1d9b5d822d67cefbf646964736f6c634300081e0033")]

    contract KarmaTiers is Ownable {
        /// @notice Emitted when a tier list is updated
        event TiersUpdated();
        /// @notice Emitted when a transaction amount is invalid

        error InvalidTxAmount();
        /// @notice Emitted when a tier name is empty
        error EmptyTierName();
        /// @notice Emitted when a tier array is empty
        error EmptyTiersArray();
        /// @notice Emitted when a tier is not found
        error TierNotFound();
        /// @notice Emitted when a tier name exceeds maximum length
        error TierNameTooLong(uint256 nameLength, uint256 maxLength);
        /// @notice Emitted when tiers are not contiguous
        error NonContiguousTiers(uint8 index, uint256 expectedMinKarma, uint256 actualMinKarma);
        /// @notice Emitted when a tier's minKarma is greater than or equal to maxKarma
        error InvalidTierRange(uint256 minKarma, uint256 maxKarma);

        struct Tier {
            uint256 minKarma;
            uint256 maxKarma;
            string name;
            uint32 txPerEpoch;
        }

        modifier onlyValidTierId(uint8 tierId) {
            if (tierId >= tiers.length) {
                revert TierNotFound();
            }
            _;
        }

        uint256 public constant MAX_TIER_NAME_LENGTH = 32;

        Tier[] public tiers;

        constructor() {
            transferOwnership(msg.sender);
        }

        function updateTiers(Tier[] calldata newTiers) external onlyOwner {
            if (newTiers.length == 0) {
                revert EmptyTiersArray();
            }
            // Ensure the first tier starts at minKarma = 0
            if (newTiers[0].minKarma != 0) {
                revert NonContiguousTiers(0, 0, newTiers[0].minKarma);
            }

            delete tiers; // Clear existing tiers

            uint256 lastMaxKarma = 0;
            for (uint8 i = 0; i < newTiers.length; i++) {
                Tier calldata input = newTiers[i];

                _validateTierName(input.name);
                if (input.maxKarma <= input.minKarma) {
                    revert InvalidTierRange(input.minKarma, input.maxKarma);
                }

                if (i > 0) {
                    uint256 expectedMinKarma = lastMaxKarma + 1;
                    if (input.minKarma != expectedMinKarma) {
                        revert NonContiguousTiers(i, expectedMinKarma, input.minKarma);
                    }
                }
                lastMaxKarma = input.maxKarma;
                tiers.push(input);
            }

            emit TiersUpdated();
        }

        function _validateTierName(string calldata name) internal pure {
            bytes memory nameBytes = bytes(name);
            if (nameBytes.length == 0) revert EmptyTierName();
            if (nameBytes.length > MAX_TIER_NAME_LENGTH) {
                revert TierNameTooLong(nameBytes.length, MAX_TIER_NAME_LENGTH);
            }
        }

        function getTierIdByKarmaBalance(uint256 karmaBalance) external view returns (uint8) {
            for (uint8 i = 0; i < tiers.length; i++) {
                if (karmaBalance < tiers[i].minKarma) {
                    return i - 1; // Return the previous tier if this one is not met
                }
            }
            return uint8(tiers.length - 1); // If all tiers are met, return the highest tier
        }

        function getTierCount() external view returns (uint256 count) {
            return tiers.length;
        }

        function getTierById(uint8 tierId) external view onlyValidTierId(tierId) returns (Tier memory tier) {
            return tiers[tierId];
        }
    }
);

impl<P: Provider> KarmaTiers::KarmaTiersInstance<P> {
    /*
    /// Try to create a new instance with a signer
    pub async fn try_new_with_signer(
        rpc_url: Url,
        address: Address,
        private_key: String,
    ) -> Result<KarmaTiers::KarmaTiersInstance<impl alloy::providers::Provider>, KarmaTiersError>
    {
        if private_key.is_empty() {
            return Err(KarmaTiersError::EmptyPrivateKey);
        }

        let ws_connect = WsConnect::new(rpc_url.as_str());
        let signer = PrivateKeySigner::from_str(&private_key)
            .map_err(|e| KarmaTiersError::SignerConnectionError(e.to_string()))?;

        let provider = ProviderBuilder::new()
            .network::<Ethereum>()
            .wallet(signer)
            .connect_ws(ws_connect)
            .await
            .map_err(KarmaTiersError::RpcTransportError)?;

        Ok(KarmaTiers::new(address, provider))
    }
    */

    /*
    /// Read smart contract `tiers` mapping
    pub async fn get_tiers(
        ws_rpc_url: Url,
        sc_address: Address,
    ) -> Result<Vec<Tier>, KarmaTiersError> {
        let ws = WsConnect::new(ws_rpc_url.as_str());
        let provider = ProviderBuilder::new()
            .connect_ws(ws)
            .await
            .map_err(KarmaTiersError::RpcTransportError)?;

        Self::get_tiers_from_provider(&provider, &sc_address).await
    }
    */

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

        // Wait for issue: https://github.com/alloy-rs/alloy/issues/2744 to be fixed
        /*
        let get0 = CallItemBuilder::new(karma_tiers_sc.getTierById(0)); // Set the amount of eth that should be deposited into the contract.
        let get1 = CallItemBuilder::new(karma_tiers_sc.getTierById(1)); // Set the amount of eth that should be deposited into the contract.
        let multicall = provider
            .multicall()
            .add_call(get0)
            .add_call(get1)
            ;

        let (res0, res1) = multicall
            // .aggregate()
            .aggregate()
            .await
            .unwrap()
            // .map_err(GetScTiersError::Multicall)?;
           ;

        // res.into_iter()
        //     .map(|t| t.map(Tier::from))
        //     .collect::<Result<Vec<_>, _>>()
        //     .map_err(|_e| GetScTiersError::MulticallInner)
        // let res_ = res.unwrap();
        // Ok(vec![Tier::from(res_.0.unwrap()), Tier::from(res_.1.unwrap())])
        Ok(vec![])
        */

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
}

#[derive(Debug, Clone, PartialEq)]
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
    use alloy::providers::ProviderBuilder;

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
        // Spin up a forked Anvil node.
        // Ensure `anvil` is available in $PATH.
        let provider = ProviderBuilder::new().connect_anvil_with_wallet();

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

        let res = KarmaTiersInstance::get_tiers_from_provider(&provider, contract.address())
            .await
            .unwrap();
        assert_eq!(res, tiers.to_vec());
    }
}
