use std::num::TryFromIntError;
use std::string::FromUtf8Error;
// third-party
use alloy::primitives::U256;
use ark_bn254::Fr;
use ark_ff::fields::AdditiveGroup;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError};
use nom::{
    IResult, Parser,
    bytes::complete::take,
    error::{ContextError, context},
    multi::length_count,
    number::complete::{le_u32, le_u64},
};
use rln::utils::IdSecret;
use rln_proof::RlnUserIdentity;
// internal
use crate::tier::TierLimits;
use crate::user_db_types::{IndexInMerkleTree, TreeIndex};
use smart_contract::Tier;

#[derive(Clone)]
pub(crate) struct RlnUserIdentitySerializer {}

impl RlnUserIdentitySerializer {
    pub(crate) fn serialize(
        &self,
        value: &RlnUserIdentity,
        buffer: &mut Vec<u8>,
    ) -> Result<(), SerializationError> {
        buffer.resize(self.size_hint(), 0);
        let compressed_size = value.commitment.compressed_size();
        let (co_buffer, rem_buffer) = buffer.split_at_mut(compressed_size);
        value.commitment.serialize_compressed(co_buffer)?;
        let (secret_buffer, user_limit_buffer) = rem_buffer.split_at_mut(compressed_size);
        value.secret_hash.serialize_compressed(secret_buffer)?;
        value.user_limit.serialize_compressed(user_limit_buffer)?;
        Ok(())
    }

    pub(crate) fn size_hint(&self) -> usize {
        Fr::ZERO.compressed_size() * 3
    }
}

#[derive(Clone)]
pub(crate) struct RlnUserIdentityDeserializer {}

impl RlnUserIdentityDeserializer {
    pub(crate) fn deserialize(&self, buffer: &[u8]) -> Result<RlnUserIdentity, SerializationError> {
        let compressed_size = Fr::ZERO.compressed_size();
        let (co_buffer, rem_buffer) = buffer.split_at(compressed_size);
        let commitment: Fr = CanonicalDeserialize::deserialize_compressed(co_buffer)?;
        let (secret_buffer, user_limit_buffer) = rem_buffer.split_at(compressed_size);
        let mut secret_hash_: Fr = CanonicalDeserialize::deserialize_compressed(secret_buffer)?;
        let secret_hash = IdSecret::from(&mut secret_hash_);
        let user_limit: Fr = CanonicalDeserialize::deserialize_compressed(user_limit_buffer)?;

        Ok({
            RlnUserIdentity {
                commitment,
                secret_hash,
                user_limit,
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct U64Serializer {}

impl U64Serializer {
    pub(crate) fn serialize(&self, value: &u64, buffer: &mut Vec<u8>) {
        buffer.extend(value.to_le_bytes());
    }

    pub(crate) const fn size_hint(&self) -> usize {
        size_of::<u64>()
    }
}

#[derive(Clone)]
pub(crate) struct U64Deserializer {}

impl U64Deserializer {
    pub(crate) fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], u64, nom::error::Error<&'a [u8]>> {
        le_u64(buffer)
    }
}

#[derive(Clone)]
pub(crate) struct TreeIndexSerializer {}

impl TreeIndexSerializer {
    pub(crate) fn serialize(&self, value: &TreeIndex, buffer: &mut Vec<u8>) {
        let value: u64 = (*value).into();
        buffer.extend(value.to_le_bytes());
    }

    pub(crate) const fn size_hint(&self) -> usize {
        // Note: Assume usize size == 8 bytes
        size_of::<TreeIndex>()
    }
}

#[derive(Clone)]
pub(crate) struct TreeIndexDeserializer {}

impl TreeIndexDeserializer {
    pub(crate) fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], TreeIndex, nom::error::Error<&'a [u8]>> {
        le_u64(buffer).map(|(input, idx)| (input, TreeIndex::from(idx)))
    }
}

#[derive(Clone)]
pub(crate) struct IndexInMerkleTreeSerializer {}

impl IndexInMerkleTreeSerializer {
    pub(crate) fn serialize(&self, value: &IndexInMerkleTree, buffer: &mut Vec<u8>) {
        let value: u64 = (*value).into();
        buffer.extend(value.to_le_bytes());
    }

    pub(crate) const fn size_hint(&self) -> usize {
        // Note: Assume usize size == 8 bytes
        size_of::<IndexInMerkleTree>()
    }
}

#[derive(Clone)]
pub(crate) struct IndexInMerkleTreeDeserializer {}

impl IndexInMerkleTreeDeserializer {
    pub(crate) fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], IndexInMerkleTree, nom::error::Error<&'a [u8]>> {
        le_u64(buffer).map(|(input, idx)| (input, IndexInMerkleTree::from(idx)))
    }
}

#[derive(Default, Clone)]
pub(crate) struct TierSerializer {}

impl TierSerializer {
    pub(crate) fn serialize(
        &self,
        value: &Tier,
        buffer: &mut Vec<u8>,
    ) -> Result<(), TryFromIntError> {
        const U256_SIZE: usize = size_of::<U256>();
        buffer.extend(value.min_karma.to_le_bytes::<U256_SIZE>().as_slice());
        buffer.extend(value.max_karma.to_le_bytes::<U256_SIZE>().as_slice());

        let name_len = u32::try_from(value.name.len())?;
        buffer.extend(name_len.to_le_bytes());
        buffer.extend(value.name.as_bytes());
        buffer.extend(value.tx_per_epoch.to_le_bytes().as_slice());
        Ok(())
    }

    pub(crate) fn size_hint(&self) -> usize {
        size_of::<Tier>()
    }
}

#[derive(Default, Clone)]
pub(crate) struct TierDeserializer {}

#[derive(Debug, PartialEq)]
pub enum TierDeserializeError<I> {
    Utf8Error(FromUtf8Error),
    TryFrom,
    Nom(I, nom::error::ErrorKind),
}

impl<I> nom::error::ParseError<I> for TierDeserializeError<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        TierDeserializeError::Nom(input, kind)
    }

    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<I> ContextError<I> for TierDeserializeError<I> {}

impl TierDeserializer {
    pub(crate) fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], Tier, TierDeserializeError<&'a [u8]>> {
        let (input, min_karma) = take(32usize)(buffer)?;
        let min_karma = U256::try_from_le_slice(min_karma)
            .ok_or(nom::Err::Error(TierDeserializeError::TryFrom))?;
        let (input, max_karma) = take(32usize)(input)?;
        let max_karma = U256::try_from_le_slice(max_karma)
            .ok_or(nom::Err::Error(TierDeserializeError::TryFrom))?;
        let (input, name_len) = le_u32(input)?;
        let name_len_ = usize::try_from(name_len)
            .map_err(|_e| nom::Err::Error(TierDeserializeError::TryFrom))?;
        let (input, name) = take(name_len_)(input)?;
        let name = String::from_utf8(name.to_vec())
            .map_err(|e| nom::Err::Error(TierDeserializeError::Utf8Error(e)))?;
        let (input, tx_per_epoch) = le_u32(input)?;

        Ok((
            input,
            Tier {
                min_karma,
                max_karma,
                name,
                tx_per_epoch,
            },
        ))
    }
}

#[derive(Default, Clone)]
pub(crate) struct TierLimitsSerializer {
    tier_serializer: TierSerializer,
}

impl TierLimitsSerializer {
    pub(crate) fn serialize(
        &self,
        value: &TierLimits,
        buffer: &mut Vec<u8>,
    ) -> Result<(), TryFromIntError> {
        let len = value.len() as u32;
        buffer.extend(len.to_le_bytes());
        let mut tier_buffer = Vec::with_capacity(self.tier_serializer.size_hint());
        value.iter().try_for_each(|t| {
            self.tier_serializer.serialize(t, &mut tier_buffer)?;
            buffer.extend_from_slice(&tier_buffer);
            tier_buffer.clear();
            Ok(())
        })
    }

    pub(crate) fn size_hint(&self, len: usize) -> usize {
        size_of::<u32>() + len * self.tier_serializer.size_hint()
    }
}

#[derive(Default, Clone)]
pub(crate) struct TierLimitsDeserializer {
    pub(crate) tier_deserializer: TierDeserializer,
}

impl TierLimitsDeserializer {
    pub(crate) fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], TierLimits, TierDeserializeError<&'a [u8]>> {
        let (input, tiers): (&[u8], Vec<Tier>) = length_count(
            le_u32,
            context("Tier index & Tier deser", |input: &'a [u8]| {
                let (input, tier) = self.tier_deserializer.deserialize(input)?;
                Ok((input, tier))
            }),
        )
        .map(Vec::from_iter)
        .parse(buffer)?;

        Ok((input, TierLimits::from(tiers)))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_rln_ser_der() {
        let rln_user_identity = RlnUserIdentity {
            commitment: Fr::from(42),
            secret_hash: IdSecret::from(&mut Fr::from(u16::MAX)),
            user_limit: Fr::from(1_000_000),
        };
        let serializer = RlnUserIdentitySerializer {};
        let mut buffer = Vec::with_capacity(serializer.size_hint());
        serializer
            .serialize(&rln_user_identity, &mut buffer)
            .unwrap();

        let deserializer = RlnUserIdentityDeserializer {};
        let de = deserializer.deserialize(&buffer).unwrap();

        assert_eq!(rln_user_identity, de);
    }

    #[test]
    fn test_index_mtree_ser_der() {
        let index = IndexInMerkleTree::from(4242u64);

        let serializer = IndexInMerkleTreeSerializer {};
        let mut buffer = Vec::with_capacity(serializer.size_hint());
        serializer.serialize(&index, &mut buffer);

        let deserializer = IndexInMerkleTreeDeserializer {};
        let (_, de) = deserializer.deserialize(&buffer).unwrap();

        assert_eq!(index, de);
    }

    #[test]
    fn test_tier_ser_der() {
        let tier = Tier {
            min_karma: U256::from(10),
            max_karma: U256::from(u64::MAX),
            name: "All".to_string(),
            tx_per_epoch: 10_000_000,
        };

        let serializer = TierSerializer {};
        let mut buffer = Vec::with_capacity(serializer.size_hint());
        serializer.serialize(&tier, &mut buffer).unwrap();

        let deserializer = TierDeserializer {};
        let (_, de) = deserializer.deserialize(&buffer).unwrap();

        assert_eq!(tier, de);
    }

    #[test]
    fn test_tier_limits_ser_der() {
        let tier_1 = Tier {
            min_karma: U256::from(2),
            max_karma: U256::from(4),
            name: "Basic".to_string(),
            tx_per_epoch: 10_000,
        };
        let tier_2 = Tier {
            min_karma: U256::from(10),
            max_karma: U256::from(u64::MAX),
            name: "Premium".to_string(),
            tx_per_epoch: 1_000_000_000,
        };

        let tier_limits = TierLimits::from([tier_1, tier_2]);

        let serializer = TierLimitsSerializer::default();
        let mut buffer = Vec::with_capacity(serializer.size_hint(tier_limits.len()));
        serializer.serialize(&tier_limits, &mut buffer).unwrap();

        let deserializer = TierLimitsDeserializer::default();
        let (_, de) = deserializer.deserialize(&buffer).unwrap();

        assert_eq!(tier_limits, de);
    }
}
