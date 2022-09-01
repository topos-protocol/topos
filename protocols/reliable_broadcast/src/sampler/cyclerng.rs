#[allow(unused)]
use rand::Rng;

#[cfg(not(test))]
pub use rand::distributions::{Distribution, Uniform};
#[cfg(not(test))]
// #[allow(unused)]
pub use rand::thread_rng;

#[cfg(test)]
pub use rand::distributions::Distribution;

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct Uniform {
    pub testing: String,
    pub low: usize,
    pub range: usize,
}

#[cfg(test)]
impl Uniform {
    pub fn new(low: usize, high: usize) -> Self {
        Uniform {
            testing: "this is a mock implementation!".to_owned(),
            low,
            range: high - low,
        }
    }

    pub fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> usize {
        _rng.gen()
    }
}

#[cfg(test)]
use byteorder::{ByteOrder, LittleEndian};

#[cfg(test)]
thread_local! {
    static CYCLE_IDX: std::cell::RefCell<usize> = std::cell::RefCell::new(0);
    static CYCLE_RNG: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());
}

#[cfg(test)]
pub mod utils {
    use super::*;

    pub fn reset_idx() {
        CYCLE_IDX.with(|i| {
            *i.borrow_mut() = 0;
        });
    }

    pub fn set_cycle<T>(values: T)
    where
        T: IntoIterator<Item = u64>,
    {
        CYCLE_RNG.with(|v| {
            *v.borrow_mut() = values.into_iter().collect();
        });
        utils::reset_idx();
    }

    #[allow(dead_code)]
    pub fn convert_bytes_to_u64(data: &[u8]) -> Vec<u64> {
        let mut vec64 = Vec::<u64>::with_capacity(data.len() / 8);
        LittleEndian::read_u64_into(data, &mut vec64);
        vec64
    }
}

#[cfg(test)]
pub fn thread_rng() -> impl rand::RngCore {
    #[derive(Clone, Copy)]
    struct CycleRng;

    // adapted from https://github.com/rust-random/rand/blob/master/src/rngs/mock.rs
    impl rand::RngCore for CycleRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            CYCLE_IDX.with(|i| {
                let mut idx = *i.borrow();
                CYCLE_RNG.with(|v| {
                    let values = v.borrow();
                    let len = values.len();
                    if len == 0 {
                        panic!("Use utils::set_cycle to seed values into the cyclerng");
                    }
                    if idx >= len {
                        idx = 0;
                    }
                    let result = values[idx];
                    idx += 1;
                    *i.borrow_mut() = idx;
                    result
                })
            })
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            rand_core::impls::fill_bytes_via_next(self, dest);
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    CycleRng
}

#[cfg(test)]
mod should {
    use super::*;
    use rand::RngCore;

    #[test]
    fn read_and_write_little_endian_bytes_and_u64() {
        let mut bytes = [0; 32];
        let numbers_given = [1, 2, 0xf00f, 0xffee];
        LittleEndian::write_u64_into(&numbers_given, &mut bytes);

        let mut numbers_got = [0; 4];
        LittleEndian::read_u64_into(&bytes, &mut numbers_got);
        assert_eq!(numbers_given, numbers_got);
    }

    #[test]
    #[should_panic(expected = "Use utils::set_cycle to seed values into the cyclerng")]
    fn panic_empty_cycle() {
        thread_rng().next_u64();
    }

    #[test]
    fn return_value_provided_in_set_cycle() {
        utils::set_cycle([1]);
        let actual = thread_rng().next_u64();
        assert_eq!(1, actual);
    }

    #[test]
    fn fill_bytes_from_set_values() {
        let numbers_given = [4, 5, 6];
        // Set the values using u64
        utils::set_cycle(numbers_given);
        let mut actual = [0u8; 48];
        // Fill the buffer with bytes from the rng
        thread_rng().try_fill_bytes(&mut actual).unwrap();
        // assert_eq!(actual, [0; 48]); // To see the byte output uncomment this liine
        let mut expected_bytes = [0; 48];
        // Convert the u64 to bytes to compare the output
        LittleEndian::write_u64_into(&numbers_given, &mut expected_bytes[..24]);
        LittleEndian::write_u64_into(&numbers_given, &mut expected_bytes[24..]);
        assert_eq!(expected_bytes, actual);
    }

    #[test]
    fn reset_idx_when_reset_idx_called() {
        utils::set_cycle([10, 20, 30]);
        assert_eq!(10, thread_rng().next_u64());
        assert_eq!(20, thread_rng().next_u64());
        utils::reset_idx();
        assert_eq!(10, thread_rng().next_u64());
        assert_eq!(20, thread_rng().next_u64());
    }

    #[test]
    fn reset_idx_when_set_cycle_called() {
        utils::set_cycle([10, 20, 30]);
        assert_eq!(10, thread_rng().next_u64());
        assert_eq!(20, thread_rng().next_u64());
        utils::set_cycle([100, 200, 300]);
        assert_eq!(100, thread_rng().next_u64());
        assert_eq!(200, thread_rng().next_u64());
    }
}

/// Simple example of a wrapper struct using the 'thread_rng()'
#[allow(unused)]
mod example {
    use super::*;
    use rand::Rng;

    pub struct RandGen {}

    impl RandGen {
        pub fn new() -> Self {
            RandGen {}
        }

        pub fn rand(&mut self) -> u32 {
            thread_rng().gen()
        }
    }
}
