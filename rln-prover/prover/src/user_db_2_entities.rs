use ark_bn254::Fr;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rln_proof::RlnUserIdentity;
use sqlx::postgres::types::Oid;
use sqlx::postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef};
use sqlx::{Decode, Encode, Postgres, Type};
use std::sync::OnceLock;
// internal
use crate::tier::TierLimits;

#[derive(Debug, sqlx::FromRow)]
pub struct UserIdSqlx {
    pub id: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserSqlx {
    pub id: i64, // primary key
    pub address: Vec<u8>,
    pub rln_id: sqlx::types::Json<RlnUserIdentity>,
    pub tree_index: i64,
    pub index_in_merkle_tree: i64,
}

#[derive(sqlx::FromRow)]
pub struct TierLimitsSqlx {
    pub id: i64,      // primary key
    pub name: String, // unique
    pub tier_limits: Option<sqlx::types::Json<TierLimits>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TxCounterSqlx {
    pub id: i64,
    pub address: Vec<u8>, // unique
    pub epoch: i64,
    pub epoch_counter: i64,
    pub quota_bonus: i64,
}

#[derive(sqlx::FromRow)]
pub struct MerkleTreeConfigSqlx {
    pub id: i64,
    pub tree_index: i16, // unique
    pub depth: i64,
    pub next_index: i64,
}

#[derive(sqlx::FromRow, Debug)]
pub struct DenyListSqlx {
    pub address: Vec<u8>, // unique
    pub expires_at: Option<i64>,
    pub denied_at: Option<i64>,
}

#[derive(sqlx::FromRow)]
pub struct NullifierSqlx {
    pub nullifier: Vec<u8>, // primary key (part 1)
    pub epoch: i64,         // primary key (part 2)
}

// sqlx custom types

// Cache for pgfr oid
pub static PGFR_OID: OnceLock<Oid> = OnceLock::new();
// Cache for pgfr array oid (type: _pgrf)
// Note:
// Postgres automatically creates an array type named with an underscore usually.
pub static PGFR_ARRAY_OID: OnceLock<Oid> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct PgFrStruct {
    pub inner: Fr,
}

impl Type<Postgres> for PgFrStruct {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        // sqlx::postgres::PgTypeInfo::with_name("pgfr")
        let oid = *PGFR_OID
            .get()
            .expect("PGFR_OID must be initialized in main()");
        PgTypeInfo::with_oid(oid)
    }
}

impl<'q> Encode<'q, Postgres> for PgFrStruct {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let mut temp_buf = Vec::with_capacity(32);
        self.inner.serialize_compressed(&mut temp_buf)?;
        buf.extend_from_slice(&temp_buf);
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'r> Decode<'r, Postgres> for PgFrStruct {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = value.as_bytes()?;
        let fr = Fr::deserialize_compressed(bytes)?;
        Ok(PgFrStruct { inner: fr })
    }
}

impl PgHasArrayType for PgFrStruct {
    fn array_type_info() -> PgTypeInfo {
        // PgTypeInfo::with_name("_pgfr")
        let oid = *PGFR_ARRAY_OID
            .get()
            .expect("PGFR_ARRAY_OID must be initialized");
        PgTypeInfo::with_oid(oid)
    }
}

#[derive(Default, Debug)]
pub struct MerkleProof {
    pub inner: Vec<(i64, Fr)>,
}

impl Type<Postgres> for MerkleProof {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("bytea")
    }
}

impl<'r> Decode<'r, Postgres> for MerkleProof {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = <&[u8] as Decode<Postgres>>::decode(value)?;
        let inner_ = Vec::<(u64, Fr)>::deserialize_compressed(bytes)?;

        // Note: ark-serialize 0.5 only supports u64 so we need to convert to i64
        // FIXME: update ark-serialize in zerokit crate or wait for a new release
        let inner: Vec<(i64, Fr)> = inner_
            .into_iter()
            .map(|(index, value)| (index as i64, value))
            .collect();

        Ok(MerkleProof { inner })
    }
}
