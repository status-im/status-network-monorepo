mod merkle_tree;
mod poseidon;
mod merkle_tree_utils;

// std
use std::ffi::CStr;
use std::str::FromStr;
// third-party
use ark_bn254::Fr;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_ff::{BigInteger, PrimeField};
// pgrx
use pgrx::{
    datum::Datum,
    callconv::{ArgAbi, BoxRet},
    rust_regtypein,
    StringInfo,
    pgrx_sql_entity_graph::metadata::{ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable},
    prelude::*,
    pg_sys::Oid,
    // pg_sys::panic::ErrorReportable,
    // pg_test
};

::pgrx::pg_module_magic!(name, version);

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
struct PgFr(Fr);

extension_sql!(
    r#"CREATE TYPE pgfr;"#,
    name = "create_pgfr_shell_type",
    creates = [Type(PgFr2)]
);

unsafe impl SqlTranslatable for PgFr {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::literal("pgfr"))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::literal("pgfr")))
    }
}

#[pg_extern(immutable, strict)]
fn pgfr_in(input: &CStr) -> PgFr {

    // warning!("pgfr_in");
    let input_as_str = input.to_str().expect("Unable to convert CStr to str");

    match Fr::from_str(input_as_str) {
        Ok(fr) => PgFr(fr),
        Err(_err) => {
            ereport!(
                ERROR,
                PgSqlErrorCode::ERRCODE_INVALID_TEXT_REPRESENTATION,
                format!("invalid input syntax for type fr: '{input_as_str}'")
            );
        }
    }
}

#[pg_extern(immutable)]
fn pgfr_out(pg_fr: PgFr) -> &'static CStr {
    // warning!("pgfr_out");
    let mut sb = StringInfo::new();
    sb.push_str(pg_fr.0.to_string().as_str());
    unsafe { sb.leak_cstr() }
}

#[pg_extern(immutable, strict, parallel_safe)]
fn pgfr_send(val: PgFr) -> Vec<u8> {
    // warning!("pgfr_send");
    let mut buffer = Vec::with_capacity(32);
    match val.0.serialize_compressed(&mut buffer) {
        Ok(()) => buffer,
        Err(e) => {
            ereport!(
                ERROR,
                PgSqlErrorCode::ERRCODE_INVALID_BINARY_REPRESENTATION,
                format!("Failed to serialize Fr: {}", e)
            );
        }
    }
}

#[pg_extern(immutable, strict, parallel_safe)]
fn pgfr_recv(mut internal: ::pgrx::datum::Internal) -> PgFr {

    // warning!("[pgfr_recv] internal: {:?}", internal.initialized());
    let buf = unsafe { internal.get_mut::<::pgrx::pg_sys::StringInfoData>().unwrap() };
    buf.cursor = buf.len;
    let bytes = unsafe {
        core::slice::from_raw_parts(buf.data as *const u8, buf.len as usize)
    };
    match Fr::deserialize_compressed(bytes) {
        Ok(fr) => PgFr(fr),
        Err(e) => {
            ereport!(
                ERROR,
                PgSqlErrorCode::ERRCODE_INVALID_BINARY_REPRESENTATION,
                format!("Failed to deserialize Fr: {}", e)
            );
        }
    }
}

impl FromDatum for PgFr {
    unsafe fn from_polymorphic_datum(datum: pg_sys::Datum, is_null: bool, _typoid: Oid) -> Option<Self>
    where
        Self: Sized,
    {
        // warning!("from_poly_datum");
        if is_null {
            None
        } else {
            let ptr = datum.cast_mut_ptr::<u8>();
            let bytes = std::slice::from_raw_parts(ptr, 32);
            match Fr::deserialize_compressed(bytes) {
                Ok(fr) => Some(PgFr(fr)),
                Err(_) => {
                    error!("Failed to deserialize PgFr from disk storage");
                }
            }
        }
    }
}

impl IntoDatum for PgFr {
    fn into_datum(self) -> Option<pg_sys::Datum> {
        // warning!("[PgFr] into_datum: {:?}", self);
        // TODO / FIXME: should use serialized_compressed here?
        let bytes = self.0.into_bigint().to_bytes_le();
        unsafe {
            let ptr = pg_sys::palloc(32);
            std::ptr::copy_nonoverlapping(bytes.as_slice().as_ptr(), ptr as *mut u8, 32);
            Some(pg_sys::Datum::from(ptr as usize))
        }
    }

    fn type_oid() -> Oid {
        // warning!("type_oid: {}", rust_regtypein::<Self>());
        rust_regtypein::<Self>()
    }
}

unsafe impl<'fcx> ArgAbi<'fcx> for PgFr
where
    Self: 'fcx,
{
    unsafe fn unbox_arg_unchecked(arg: ::pgrx::callconv::Arg<'_, 'fcx>) -> Self {
        // warning!("unbox_arg");
        unsafe { arg.unbox_arg_using_from_datum().unwrap() }
    }
}

unsafe impl BoxRet for PgFr {
    unsafe fn box_into<'fcx>(self, fcinfo: &mut pgrx::callconv::FcInfo<'fcx>) -> Datum<'fcx> {
        // warning!("box_into");
        unsafe { fcinfo
            .return_raw_datum( self.into_datum().unwrap() ) }
    }
}

extension_sql!(
    r#"
CREATE TYPE pgfr (
   internallength = 32,
   input = pgfr_in,
   output = pgfr_out,
   send = pgfr_send,
   receive = pgfr_recv,
   alignment = double
);
"#,
    name = "create_pgfr_type",
    requires = ["create_pgfr_shell_type",
        pgfr_in, pgfr_out,
        pgfr_send, pgfr_recv
    ],
);

#[pg_extern(immutable, parallel_safe)]
fn pgfr_to_bytea(input: PgFr) -> Vec<u8> {
    let mut result = Vec::with_capacity(32);
    input.0.serialize_compressed(&mut result).unwrap();
    result
}

#[pg_extern(immutable, parallel_safe)]
fn bytea_to_pgfr(input: &[u8]) -> PgFr {
    if input.len() != 32 {
        ereport!(
            ERROR,
            PgSqlErrorCode::ERRCODE_INVALID_BINARY_REPRESENTATION,
            format!("Bytea Cast - pgfr requires exactly 32 bytes, received {} bytes", input.len())
        );
    }
    match Fr::deserialize_compressed(input) {
        Ok(fr) => PgFr(fr),
        Err(e) => {
            ereport!(
                ERROR,
                PgSqlErrorCode::ERRCODE_INVALID_BINARY_REPRESENTATION,
                format!("Bytea Cast - Invalid encoding for pgfr: {}", e)
            );
        }
    }
}

// Note: AS ASSIGNEMENT -> cast can be invoked only in assignment contexts
// Check: https://www.postgresql.org/docs/18/sql-createcast.html
extension_sql!(
    r#"
CREATE CAST (pgfr AS bytea) WITH FUNCTION pgfr_to_bytea(pgfr) AS ASSIGNMENT;
CREATE CAST (bytea AS pgfr) WITH FUNCTION bytea_to_pgfr(bytea) AS ASSIGNMENT;
"#,
    name = "pgfr_casts",
    requires = [
        pgfr_to_bytea,
        bytea_to_pgfr
    ]
);

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use static_assertions::const_assert_eq;
    use std::ffi::c_void;
    use super::*;

    const _: () = {
        const_assert_eq!(std::mem::size_of::<PgFr>(), 32);
    };

    #[pg_test]
    fn test_pgfr2_is_32_bytes() {
        let length: i32 = Spi::get_one("SELECT pg_column_size('2'::pgfr);")
            .expect("pg_column_size to not fail")
            .expect("length not null");
        assert_eq!(length, 32);
    }

    #[pg_test]
    fn test_pgfr_init_text() {
        // Check we can init a pgfr using TEXT
        // Init table + set value
        let _res = Spi::run("
            CREATE TABLE test_pgfr (my_index bigint, value pgfr);
            INSERT INTO test_pgfr (my_index, value) VALUES (0, '2');
            INSERT INTO test_pgfr (my_index, value) VALUES (1, '42');
            "
        );

        let res_0 = Spi::get_one::<PgFr>("SELECT value FROM test_pgfr WHERE my_index = 0;")
            .expect("Table is empty")
            .expect("Value is null");
        assert_eq!(res_0.0, Fr::from(2));
        let res_1 = Spi::get_one::<PgFr>("SELECT value FROM test_pgfr WHERE my_index = 1;")
            .expect("Table is empty")
            .expect("Value is null");
        assert_eq!(res_1.0, Fr::from(42));
    }

    #[pg_test]
    fn test_pgfr_init_bytea() {
        // Check we can init a pgfr using BYTEA
        // Init table + set value
        let _res = Spi::run("
            CREATE TABLE test_pgfr (my_index bigint, value pgfr);
            INSERT INTO test_pgfr (my_index, value) VALUES (0, '\\x0200000000000000000000000000000000000000000000000000000000000000'::bytea);
            INSERT INTO test_pgfr (my_index, value) VALUES (1, '\\x2a00000000000000000000000000000000000000000000000000000000000000'::bytea);
            "
        );

        let res_0 = Spi::get_one::<PgFr>("SELECT value FROM test_pgfr WHERE my_index = 0;")
            .expect("Table is empty")
            .expect("Value is null");
        assert_eq!(res_0.0, Fr::from(2));
        let res_1 = Spi::get_one::<PgFr>("SELECT value FROM test_pgfr WHERE my_index = 1;")
            .expect("Table is empty")
            .expect("Value is null");
        assert_eq!(res_1.0, Fr::from(42));
    }

    #[pg_test]
    #[should_panic(expected = "Bytea Cast - pgfr requires exactly 32 bytes, received 33 bytes")]
    fn test_pgfr_init_bytea_fail() {
        // Check we can init a pgfr using BYTEA
        // Note: 2nd bytea is 33 bytes (and not 32 bytes for Fr)
        let _res = Spi::run("
            CREATE TABLE test_pgfr (my_index bigint, value pgfr);
            INSERT INTO test_pgfr (my_index, value) VALUES (0, '\\x0200000000000000000000000000000000000000000000000000000000000000'::bytea);
            INSERT INTO test_pgfr (my_index, value) VALUES (1, '\\x2a0000000000000000000000000000000000000000000000000000000000000000'::bytea);
            "
        ).unwrap();
    }

    #[pg_test]
    fn test_pgfr_send() {
        let original_fr = Fr::from(42);
        let input = PgFr(original_fr);
        let bytes = pgfr_send(input);
        assert_eq!(bytes.len(), 32);
        let deserialized = Fr::deserialize_compressed(&bytes[..]).unwrap();
        assert_eq!(deserialized, original_fr);
    }

    #[pg_test]
    unsafe fn test_pgfr2_recv() {

        let expected_fr = Fr::from(999);
        let mut raw_bytes = Vec::new();
        expected_fr.serialize_compressed(&mut raw_bytes).unwrap();
        let string_info_ptr = pg_sys::makeStringInfo();
        // Rust doc for 2nd arg should be *const i8 but compilo requires *const c_void...
        pg_sys::appendBinaryStringInfo(
            string_info_ptr,
            raw_bytes.as_ptr() as *const c_void,
            raw_bytes.len() as i32
        );
        let datum = pg_sys::Datum::from(string_info_ptr as usize);
        let internal = pgrx::datum::Internal::from_datum(datum, false)
            .expect("Failed to create Internal from Datum");

        let result = pgfr_recv(internal);
        assert_eq!(result.0, expected_fr);
    }
}

#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}