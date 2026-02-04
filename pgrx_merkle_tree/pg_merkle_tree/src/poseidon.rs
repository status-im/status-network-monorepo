use ark_bn254::Fr;
use once_cell::sync::Lazy;
use ark_ff::PrimeField;
use num_bigint::BigUint;

pub struct PoseidonGrainLFSR {
    pub prime_num_bits: u64,
    pub state: [bool; 80],
    pub head: usize,
}

impl PoseidonGrainLFSR {
    pub fn new(
        is_field: u64,
        is_sbox_an_inverse: u64,
        prime_num_bits: u64,
        state_len: u64,
        num_full_rounds: u64,
        num_partial_rounds: u64,
    ) -> Self {
        let mut state = [false; 80];

        // Only fields are supported for now
        assert!(is_field == 1);

        // b0, b1 describes the field
        state[1] = is_field == 1;

        assert!(is_sbox_an_inverse == 0 || is_sbox_an_inverse == 1);

        // b2, ..., b5 describes the S-BOX
        state[5] = is_sbox_an_inverse == 1;

        // b6, ..., b17 are the binary representation of n (prime_num_bits)
        {
            let mut cur = prime_num_bits;
            for i in (6..=17).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b18, ..., b29 are the binary representation of t (state_len, rate + capacity)
        {
            let mut cur = state_len;
            for i in (18..=29).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b30, ..., b39 are the binary representation of R_F (the number of full rounds)
        {
            let mut cur = num_full_rounds;
            for i in (30..=39).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b40, ..., b49 are the binary representation of R_P (the number of partial rounds)
        {
            let mut cur = num_partial_rounds;
            for i in (40..=49).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b50, ..., b79 are set to 1
        for item in state.iter_mut().skip(50) {
            *item = true;
        }

        let head = 0;

        let mut res = Self {
            prime_num_bits,
            state,
            head,
        };
        res.init();
        res
    }

    pub fn get_bits(&mut self, num_bits: usize) -> Vec<bool> {
        let mut res = Vec::new();

        for _ in 0..num_bits {
            // Obtain the first bit
            let mut new_bit = self.update();

            // Loop until the first bit is true
            while !new_bit {
                // Discard the second bit
                let _ = self.update();
                // Obtain another first bit
                new_bit = self.update();
            }

            // Obtain the second bit
            res.push(self.update());
        }

        res
    }

    pub fn get_field_elements_rejection_sampling<F: PrimeField>(
        &mut self,
        num_elems: usize,
    ) -> Vec<F> {
        assert_eq!(F::MODULUS_BIT_SIZE as u64, self.prime_num_bits);
        let modulus: BigUint = F::MODULUS.into();

        let mut res = Vec::new();
        for _ in 0..num_elems {
            // Perform rejection sampling
            loop {
                // Obtain n bits and make it most-significant-bit first
                let mut bits = self.get_bits(self.prime_num_bits as usize);
                bits.reverse();

                let bytes = bits
                    .chunks(8)
                    .map(|chunk| {
                        let mut result = 0u8;
                        for (i, bit) in chunk.iter().enumerate() {
                            result |= u8::from(*bit) << i
                        }
                        result
                    })
                    .collect::<Vec<u8>>();

                let value = BigUint::from_bytes_le(&bytes);

                if value < modulus {
                    res.push(F::from(value.clone()));
                    break;
                }
            }
        }
        res
    }

    pub fn get_field_elements_mod_p<F: PrimeField>(&mut self, num_elems: usize) -> Vec<F> {
        assert_eq!(F::MODULUS_BIT_SIZE as u64, self.prime_num_bits);

        let mut res = Vec::new();
        for _ in 0..num_elems {
            // Obtain n bits and make it most-significant-bit first
            let mut bits = self.get_bits(self.prime_num_bits as usize);
            bits.reverse();

            let bytes = bits
                .chunks(8)
                .map(|chunk| {
                    let mut result = 0u8;
                    for (i, bit) in chunk.iter().enumerate() {
                        result |= u8::from(*bit) << i
                    }
                    result
                })
                .collect::<Vec<u8>>();

            res.push(F::from_le_bytes_mod_order(&bytes));
        }

        res
    }

    #[inline]
    fn update(&mut self) -> bool {
        let new_bit = self.state[(self.head + 62) % 80]
            ^ self.state[(self.head + 51) % 80]
            ^ self.state[(self.head + 38) % 80]
            ^ self.state[(self.head + 23) % 80]
            ^ self.state[(self.head + 13) % 80]
            ^ self.state[self.head];
        self.state[self.head] = new_bit;
        self.head += 1;
        self.head %= 80;

        new_bit
    }

    fn init(&mut self) {
        for _ in 0..160 {
            let new_bit = self.state[(self.head + 62) % 80]
                ^ self.state[(self.head + 51) % 80]
                ^ self.state[(self.head + 38) % 80]
                ^ self.state[(self.head + 23) % 80]
                ^ self.state[(self.head + 13) % 80]
                ^ self.state[self.head];
            self.state[self.head] = new_bit;
            self.head += 1;
            self.head %= 80;
        }
    }
}

pub fn find_poseidon_ark_and_mds<F: PrimeField>(
    is_field: u64,
    is_sbox_an_inverse: u64,
    prime_bits: u64,
    rate: usize,
    full_rounds: u64,
    partial_rounds: u64,
    skip_matrices: usize,
) -> (Vec<F>, Vec<Vec<F>>) {
    let mut lfsr = PoseidonGrainLFSR::new(
        is_field,
        is_sbox_an_inverse,
        prime_bits,
        rate as u64,
        full_rounds,
        partial_rounds,
    );

    let mut ark = Vec::<F>::with_capacity((full_rounds + partial_rounds) as usize);
    for _ in 0..(full_rounds + partial_rounds) {
        let values = lfsr.get_field_elements_rejection_sampling::<F>(rate);
        for el in values {
            ark.push(el);
        }
    }

    let mut mds = Vec::<Vec<F>>::with_capacity(rate);
    mds.resize(rate, vec![F::zero(); rate]);

    // Note that we build the MDS matrix generating 2*rate elements. If the matrix built is not secure (see checks with algorithm 1, 2, 3 in reference implementation)
    // it has to be skipped. Since here we do not implement such algorithm we allow to pass a parameter to skip generations of elements giving unsecure matrixes.
    // At the moment, the skip_matrices parameter has to be generated from the reference implementation and passed to this function
    for _ in 0..skip_matrices {
        let _ = lfsr.get_field_elements_mod_p::<F>(2 * (rate));
    }

    // a qualifying matrix must satisfy the following requirements
    // - there is no duplication among the elements in x or y
    // - there is no i and j such that x[i] + y[j] = p
    // - the resultant MDS passes all the three tests

    let xs = lfsr.get_field_elements_mod_p::<F>(rate);
    let ys = lfsr.get_field_elements_mod_p::<F>(rate);

    for i in 0..(rate) {
        for (j, ys_item) in ys.iter().enumerate().take(rate) {
            mds[i][j] = (xs[i] + ys_item).inverse().unwrap();
        }
    }

    (ark, mds)
}

pub const ROUND_PARAMS: [(usize, usize, usize, usize); 8] = [
    (2, 8, 56, 0),
    (3, 8, 57, 0),
    (4, 8, 56, 0),
    (5, 8, 60, 0),
    (6, 8, 60, 0),
    (7, 8, 63, 0),
    (8, 8, 64, 0),
    (9, 8, 63, 0),
];

static POSEIDON: Lazy<Poseidon<Fr>> = Lazy::new(|| Poseidon::<Fr>::from(&ROUND_PARAMS));

pub fn poseidon_hash_(input: &[Fr]) -> Fr {
    POSEIDON
        .hash(input)
        .expect("hash with fixed input size can't fail")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundParameters<F: PrimeField> {
    pub t: usize,
    pub n_rounds_f: usize,
    pub n_rounds_p: usize,
    pub skip_matrices: usize,
    pub c: Vec<F>,
    pub m: Vec<Vec<F>>,
}

pub struct Poseidon<F: PrimeField> {
    round_params: Vec<RoundParameters<F>>,
}

impl<F: PrimeField> Poseidon<F> {
    // Loads round parameters and generates round constants
    // poseidon_params is a vector containing tuples (t, RF, RP, skip_matrices)
    // where: t is the rate (input length + 1), RF is the number of full rounds, RP is the number of partial rounds
    // and skip_matrices is a (temporary) parameter used to generate secure MDS matrices (see comments in the description of find_poseidon_ark_and_mds)
    // TODO: implement automatic generation of round parameters
    pub fn from(poseidon_params: &[(usize, usize, usize, usize)]) -> Self {
        let mut read_params = Vec::<RoundParameters<F>>::with_capacity(poseidon_params.len());

        for &(t, n_rounds_f, n_rounds_p, skip_matrices) in poseidon_params {
            let (ark, mds) = find_poseidon_ark_and_mds::<F>(
                1, // is_field = 1
                0, // is_sbox_inverse = 0
                F::MODULUS_BIT_SIZE as u64,
                t,
                n_rounds_f as u64,
                n_rounds_p as u64,
                skip_matrices,
            );
            let rp = RoundParameters {
                t,
                n_rounds_p,
                n_rounds_f,
                skip_matrices,
                c: ark,
                m: mds,
            };
            read_params.push(rp);
        }

        Poseidon {
            round_params: read_params,
        }
    }

    /*
    pub fn get_parameters(&self) -> &Vec<RoundParameters<F>> {
        &self.round_params
    }
    */

    pub fn ark(&self, state: &mut [F], c: &[F], it: usize) {
        state.iter_mut().enumerate().for_each(|(i, elem)| {
            *elem += c[it + i];
        });
    }

    pub fn sbox(&self, n_rounds_f: usize, n_rounds_p: usize, state: &mut [F], i: usize) {
        if (i < n_rounds_f / 2) || (i >= n_rounds_f / 2 + n_rounds_p) {
            state.iter_mut().for_each(|current_state| {
                let aux = *current_state;
                *current_state *= *current_state;
                *current_state *= *current_state;
                *current_state *= aux;
            })
        } else {
            let aux = state[0];
            state[0] *= state[0];
            state[0] *= state[0];
            state[0] *= aux;
        }
    }

    pub fn mix_2(&self, state: &[F], m: &[Vec<F>], state_2: &mut [F]) {
        for i in 0..state.len() {
            // Cache the row reference
            let row = &m[i];
            let mut acc = F::ZERO;
            for j in 0..state.len() {
                acc += row[j] * state[j];
            }
            state_2[i] = acc;
        }
    }

    pub fn hash(&self, inp: &[F]) -> Result<F, String> {
        // Note that the rate t becomes input length + 1; hence for length N we pick parameters with T = N + 1
        let t = inp.len() + 1;

        // We seek the index (Poseidon's round_params is an ordered vector) for the parameters corresponding to t
        let param_index = self.round_params.iter().position(|el| el.t == t);

        if inp.is_empty() || param_index.is_none() {
            return Err("No parameters found for inputs length".to_string());
        }

        let param_index = param_index.unwrap();

        let mut state = vec![F::ZERO; t];
        let mut state_2 = state.clone();
        state[1..].clone_from_slice(inp);

        for i in 0..(self.round_params[param_index].n_rounds_f
            + self.round_params[param_index].n_rounds_p)
        {
            self.ark(
                &mut state,
                &self.round_params[param_index].c,
                i * self.round_params[param_index].t,
            );
            self.sbox(
                self.round_params[param_index].n_rounds_f,
                self.round_params[param_index].n_rounds_p,
                &mut state,
                i,
            );
            self.mix_2(&state, &self.round_params[param_index].m, &mut state_2);
            std::mem::swap(&mut state, &mut state_2);
        }

        Ok(state[0])
    }
}

impl<F> Default for Poseidon<F>
where
    F: PrimeField,
{
    // Default instantiation has no round constants set. Will return an error when hashing is attempted.
    fn default() -> Self {
        Self::from(&[])
    }
}