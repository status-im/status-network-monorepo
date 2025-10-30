use crate::epoch_service::{Epoch, EpochSlice};
// use claims::debug_assert_ge;
use nom::{
    IResult,
    error::ContextError,
    number::complete::{le_i64, le_u64},
};
use rocksdb::MergeOperands;

#[derive(Debug, PartialEq)]
pub enum DeserializeError<I> {
    Nom(I, nom::error::ErrorKind),
}

impl<I> nom::error::ParseError<I> for DeserializeError<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        DeserializeError::Nom(input, kind)
    }

    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<I> ContextError<I> for DeserializeError<I> {}

#[derive(Debug, Default, PartialEq)]
pub struct EpochCounters {
    pub epoch: Epoch,
    pub epoch_slice: EpochSlice,
    pub epoch_counter: u64,
    pub epoch_slice_counter: u64,
}

pub struct EpochCounterSerializer {}

impl EpochCounterSerializer {
    fn serialize(&self, value: &EpochCounters, buffer: &mut Vec<u8>) {
        buffer.extend(value.epoch.to_le_bytes());
        buffer.extend(value.epoch_slice.to_le_bytes());
        buffer.extend(value.epoch_counter.to_le_bytes());
        buffer.extend(value.epoch_slice_counter.to_le_bytes());
    }

    pub(crate) const fn size_hint_() -> usize {
        size_of::<EpochCounters>()
    }

    #[allow(dead_code)]
    pub(crate) fn size_hint(&self) -> usize {
        Self::size_hint_()
    }

    pub const fn default() -> [u8; Self::size_hint_()] {
        [0u8; Self::size_hint_()]
    }
}

#[derive(Clone)]
pub struct EpochCounterDeserializer {}

impl EpochCounterDeserializer {
    pub fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], EpochCounters, DeserializeError<&'a [u8]>> {
        let (input, epoch) = le_i64(buffer).map(|(i, e)| (i, Epoch::from(e)))?;
        let (input, epoch_slice) = le_i64(input).map(|(i, es)| (i, EpochSlice::from(es)))?;
        let (input, epoch_counter) = le_u64(input)?;
        let (_input, epoch_slice_counter) = le_u64(input)?;
        Ok((
            input,
            EpochCounters {
                epoch,
                epoch_slice,
                epoch_counter,
                epoch_slice_counter,
            },
        ))
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct EpochIncr {
    pub epoch: Epoch,
    pub epoch_slice: EpochSlice,
    pub incr_value: u64,
}

#[derive(Clone)]
pub struct EpochIncrSerializer {}

impl EpochIncrSerializer {
    pub fn serialize(&self, value: &EpochIncr, buffer: &mut Vec<u8>) {
        buffer.extend(value.epoch.to_le_bytes());
        buffer.extend(value.epoch_slice.to_le_bytes());
        buffer.extend(value.incr_value.to_le_bytes());
    }

    pub fn size_hint(&self) -> usize {
        size_of::<u64>() * 3
    }
}

pub struct EpochIncrDeserializer {}

impl EpochIncrDeserializer {
    pub fn deserialize<'a>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], EpochIncr, DeserializeError<&'a [u8]>> {
        let (input, epoch) = le_i64(buffer).map(|(i, e)| (i, Epoch::from(e)))?;
        let (input, epoch_slice) = le_i64(input).map(|(i, es)| (i, EpochSlice::from(es)))?;
        let (input, incr_value) = le_u64(input)?;
        Ok((
            input,
            EpochIncr {
                epoch,
                epoch_slice,
                incr_value,
            },
        ))
    }
}

pub fn epoch_counters_operands(
    _key: &[u8],
    existing_val: Option<&[u8]>,
    operands: &MergeOperands,
) -> Option<Vec<u8>> {
    let counter_ser = EpochCounterSerializer {};
    let counter_deser = EpochCounterDeserializer {};
    let ser = EpochIncrSerializer {};
    let deser = EpochIncrDeserializer {};

    // Current epoch counter structure (stored in DB)
    let epoch_counter_current = counter_deser
        .deserialize(existing_val.unwrap_or_default())
        .map(|(_, c)| c)
        .unwrap_or_default();

    // Iter over merge operands (can have multiple one with DBBatch)
    let counter_value = operands.iter().fold(epoch_counter_current, |mut acc, x| {
        // Note: unwrap on EpochIncr deserialize error - serialization is done by the prover
        //       thus no error should never happen here
        let (_, epoch_incr) = deser.deserialize(x).unwrap();

        // FIXME: assert when reload from disk
        // debug_assert_ge!(epoch_incr.epoch, acc.epoch);
        debug_assert!(
            epoch_incr.epoch_slice >= acc.epoch_slice
                || epoch_incr.epoch_slice == EpochSlice::from(0)
        );

        if acc == Default::default() {
            // Default value - so this is the first time
            acc = EpochCounters {
                epoch: epoch_incr.epoch,
                epoch_slice: epoch_incr.epoch_slice,
                epoch_counter: epoch_incr.incr_value,
                epoch_slice_counter: epoch_incr.incr_value,
            }
        } else if epoch_incr.epoch != acc.epoch {
            // New epoch
            acc = EpochCounters {
                epoch: epoch_incr.epoch,
                epoch_slice: Default::default(),
                epoch_counter: epoch_incr.incr_value,
                epoch_slice_counter: epoch_incr.incr_value,
            }
        } else if epoch_incr.epoch_slice != acc.epoch_slice {
            // New epoch slice
            acc = EpochCounters {
                epoch: epoch_incr.epoch,
                epoch_slice: epoch_incr.epoch_slice,
                epoch_counter: acc.epoch_counter.saturating_add(epoch_incr.incr_value),
                epoch_slice_counter: epoch_incr.incr_value,
            }
        } else {
            acc = EpochCounters {
                epoch: acc.epoch,
                epoch_slice: acc.epoch_slice,
                epoch_counter: acc.epoch_counter.saturating_add(epoch_incr.incr_value),
                epoch_slice_counter: acc
                    .epoch_slice_counter
                    .saturating_add(epoch_incr.incr_value),
            }
        }

        acc
    });

    let mut buffer = Vec::with_capacity(ser.size_hint());
    counter_ser.serialize(&counter_value, &mut buffer);
    Some(buffer.to_vec())
}

pub fn u64_counter_operands(
    _key: &[u8],
    existing_val: Option<&[u8]>,
    operands: &MergeOperands,
) -> Option<Vec<u8>> {
    // Counter value is stored as u64
    // But value passed (in merge / merge_cf) is i64 so we can decrease or increase the counter

    let counter_current_value = if let Some(existing_val) = existing_val {
        u64::from_le_bytes(existing_val.try_into().unwrap())
    } else {
        0
    };

    let counter_value = operands.iter().fold(counter_current_value, |mut acc, x| {
        let incr_value = i64::from_le_bytes(x.try_into().unwrap());
        acc = acc.saturating_add_signed(incr_value);
        acc
    });

    Some(counter_value.to_le_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    // std
    // third-party
    use rocksdb::{DB, Options, WriteBatch};
    use tempfile::TempDir;

    #[test]
    fn test_ser_der() {
        // EpochCounter struct
        {
            let epoch_counter = EpochCounters {
                epoch: 1.into(),
                epoch_slice: 42.into(),
                epoch_counter: 12,
                epoch_slice_counter: u64::MAX,
            };

            let serializer = EpochCounterSerializer {};
            let mut buffer = Vec::with_capacity(serializer.size_hint());
            serializer.serialize(&epoch_counter, &mut buffer);

            let deserializer = EpochCounterDeserializer {};
            let (_, de) = deserializer.deserialize(&buffer).unwrap();
            assert_eq!(epoch_counter, de);
        }

        {
            let deserializer = EpochCounterDeserializer {};
            let (_, de) = deserializer
                .deserialize(EpochCounterSerializer::default().as_slice())
                .unwrap();
            assert_eq!(EpochCounters::default(), de);
        }

        // EpochIncr struct
        {
            let epoch_incr = EpochIncr {
                epoch: 1.into(),
                epoch_slice: 42.into(),
                incr_value: 1,
            };

            let serializer = EpochIncrSerializer {};
            let mut buffer = Vec::with_capacity(serializer.size_hint());
            serializer.serialize(&epoch_incr, &mut buffer);

            let deserializer = EpochIncrDeserializer {};
            let (_, de) = deserializer.deserialize(&buffer).unwrap();
            assert_eq!(epoch_incr, de);
        }
    }

    #[test]
    fn test_counter() {
        let tmp_path = TempDir::new().unwrap().path().to_path_buf();
        let options = {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.set_merge_operator("o", u64_counter_operands, u64_counter_operands);
            opts
        };
        let db = DB::open(&options, tmp_path).unwrap();
        let key_1 = "foo1";
        // let key_2 = "baz42";

        let index = 42u64;
        let buffer = index.to_le_bytes();

        let mut db_batch = WriteBatch::default();
        db_batch.merge(key_1, buffer);
        db_batch.merge(key_1, buffer);
        db.write(db_batch).unwrap();

        let get_key_1 = db.get(key_1).unwrap().unwrap();
        let value = u64::from_le_bytes(get_key_1.try_into().unwrap());

        assert_eq!(value, index * 2); // 2x merge
    }

    #[test]
    fn test_counters() {
        let tmp_path = TempDir::new().unwrap().path().to_path_buf();
        let options = {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.set_merge_operator("o", epoch_counters_operands, epoch_counters_operands);
            opts
        };
        let db = DB::open(&options, tmp_path).unwrap();
        let key_1 = "foo1";
        let key_2 = "baz42";

        let value_1 = EpochIncr {
            epoch: 0.into(),
            epoch_slice: 0.into(),
            incr_value: 2,
        };
        let epoch_incr_ser = EpochIncrSerializer {};
        let epoch_counter_deser = EpochCounterDeserializer {};

        let mut buffer = Vec::with_capacity(epoch_incr_ser.size_hint());
        epoch_incr_ser.serialize(&value_1, &mut buffer);
        let mut db_batch = WriteBatch::default();
        db_batch.merge(key_1, &buffer);
        db_batch.merge(key_1, &buffer);
        db.write(db_batch).unwrap();

        let get_key_1 = db.get(key_1).unwrap().unwrap();
        let (_, get_value_k1) = epoch_counter_deser.deserialize(&get_key_1).unwrap();

        // Applied EpochIncr 2x
        assert_eq!(get_value_k1.epoch_counter, 4);
        assert_eq!(get_value_k1.epoch_slice_counter, 4);

        let get_key_2 = db.get(key_2).unwrap();
        assert!(get_key_2.is_none());

        // new epoch slice
        {
            let value_2 = EpochIncr {
                epoch: 0.into(),
                epoch_slice: 1.into(),
                incr_value: 1,
            };

            let mut buffer = Vec::with_capacity(epoch_incr_ser.size_hint());
            epoch_incr_ser.serialize(&value_2, &mut buffer);
            db.merge(key_1, buffer).unwrap();

            let get_key_1 = db.get(key_1).unwrap().unwrap();
            let (_, get_value_2) = epoch_counter_deser.deserialize(&get_key_1).unwrap();

            assert_eq!(
                get_value_2,
                EpochCounters {
                    epoch: 0.into(),
                    epoch_slice: 1.into(),
                    epoch_counter: 5,
                    epoch_slice_counter: 1,
                }
            )
        }

        // new epoch
        {
            let value_3 = EpochIncr {
                epoch: 1.into(),
                epoch_slice: 0.into(),
                incr_value: 3,
            };

            let mut buffer = Vec::with_capacity(epoch_incr_ser.size_hint());
            epoch_incr_ser.serialize(&value_3, &mut buffer);
            db.merge(key_1, buffer).unwrap();

            let get_key_1 = db.get(key_1).unwrap().unwrap();
            let (_, get_value_3) = epoch_counter_deser.deserialize(&get_key_1).unwrap();

            assert_eq!(
                get_value_3,
                EpochCounters {
                    epoch: 1.into(),
                    epoch_slice: 0.into(),
                    epoch_counter: 3,
                    epoch_slice_counter: 3,
                }
            )
        }
    }
}
